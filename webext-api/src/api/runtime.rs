use crate::utils::call_async_fn;
use crate::{
	error::ExtensionError,
	types::{ListenerHandle, MessageSender, attach_listener},
	utils::{call_async_fn_and_de, get_api_namespace},
};
use js_sys::{Object, Promise};
use serde::{Serialize, de::DeserializeOwned};
use serde_wasm_bindgen::to_value;
use std::{future::Future, marker::PhantomData};
use wasm_bindgen::{JsValue, prelude::*};
use wasm_bindgen_futures::future_to_promise;

#[derive(Clone)]
pub struct Runtime {
	api: Object,
}

impl Runtime {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "runtime").expect("`runtime` API not available");
		Self { api }
	}

	pub async fn send_message<M: Serialize, R: DeserializeOwned>(&self, message: &M) -> Result<R, ExtensionError> {
		call_async_fn_and_de(&self.api, "sendMessage", &[to_value(message)?][..]).await
	}

	pub fn on_message<T: DeserializeOwned + 'static>(&self) -> Result<OnMessage<T>, ExtensionError> {
		Ok(OnMessage::new(get_api_namespace(&self.api, "onMessage")?))
	}

	pub async fn open_options_page(&self) -> Result<(), ExtensionError> {
		call_async_fn(&self.api, "openOptionsPage", &[]).await?;
		Ok(())
	}
}

pub struct OnMessage<T: DeserializeOwned + 'static> {
	api: Object,
	_phantom: PhantomData<T>,
}

impl<T: DeserializeOwned + 'static> OnMessage<T> {
	fn new(api: Object) -> Self {
		Self { api, _phantom: PhantomData }
	}

	pub fn add_listener(
		&self,
		mut callback: impl FnMut(T, MessageSender) + 'static,
	) -> Result<ListenerHandle<dyn FnMut(JsValue, JsValue, JsValue)>, ExtensionError> {
		attach_listener(
			&self.api,
			Closure::wrap(Box::new(move |message, sender, _| {
				if let (Ok(msg), Ok(sender)) = (serde_wasm_bindgen::from_value(message), serde_wasm_bindgen::from_value(sender)) {
					callback(msg, sender);
				}
			}) as Box<dyn FnMut(JsValue, JsValue, JsValue)>),
		)
	}

	pub fn add_listener_with_response<F, R, O>(&self, mut callback: F) -> Result<ListenerHandle<dyn FnMut(JsValue, JsValue, JsValue) -> Promise>, ExtensionError>
	where
		F: FnMut(T, MessageSender) -> R + 'static,
		R: Future<Output = Result<O, JsValue>> + 'static,
		O: Serialize,
	{
		attach_listener(
			&self.api,
			Closure::wrap(Box::new(move |message, sender, _| {
				if let (Ok(msg), Ok(sender)) = (serde_wasm_bindgen::from_value(message), serde_wasm_bindgen::from_value(sender)) {
					let future_from_callback = callback(msg, sender);
					let processing_future = async move { future_from_callback.await.and_then(|val| to_value(&val).map_err(|e| e.into())) };
					return future_to_promise(processing_future);
				}
				Promise::resolve(&JsValue::from_bool(false))
			}) as Box<dyn FnMut(JsValue, JsValue, JsValue) -> Promise>),
		)
	}
}
