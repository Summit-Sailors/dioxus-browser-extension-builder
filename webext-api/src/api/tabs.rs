use crate::{
	error::ExtensionError,
	types::{ListenerHandle, TabChangeInfo, TabInfo, attach_listener},
	utils::{call_async_fn, call_async_fn_and_de, get_api_namespace},
};
use js_sys::Object;
use serde::{Serialize, de::DeserializeOwned};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::{JsCast, prelude::*};

#[derive(Clone)]
pub struct Tabs {
	api: Object,
}

impl Tabs {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "tabs").expect("`tabs` API not available");
		Self { api }
	}

	pub async fn get_active(&self) -> Result<TabInfo, ExtensionError> {
		let query = Object::new();
		js_sys::Reflect::set(&query, &"active".into(), &true.into())?;
		js_sys::Reflect::set(&query, &"currentWindow".into(), &true.into())?;
		let tabs = call_async_fn(&self.api, "query", &[query.into()][..]).await?;
		let tabs_array: js_sys::Array = tabs.dyn_into()?;
		if let Some(tab) = tabs_array.iter().next() { serde_wasm_bindgen::from_value(tab).map_err(Into::into) } else { Err(ExtensionError::TabNotFound) }
	}

	pub async fn send_message<M: Serialize, R: DeserializeOwned>(&self, tab_id: u32, message: &M) -> Result<R, ExtensionError> {
		call_async_fn_and_de(&self.api, "sendMessage", &[tab_id.into(), to_value(message)?][..]).await
	}

	pub fn on_updated(&self) -> Result<OnTabUpdated, ExtensionError> {
		Ok(OnTabUpdated(get_api_namespace(&self.api, "onUpdated")?))
	}
}

pub struct OnTabUpdated(Object);

impl OnTabUpdated {
	pub fn add_listener(
		&self,
		mut callback: impl FnMut(u32, TabChangeInfo, TabInfo) + 'static,
	) -> Result<ListenerHandle<dyn FnMut(JsValue, JsValue, JsValue)>, ExtensionError> {
		attach_listener(
			&self.0,
			Closure::wrap(Box::new(move |tab_id: JsValue, change_info: JsValue, tab: JsValue| {
				if let (Some(id), Ok(ci), Ok(t)) = (tab_id.as_f64(), serde_wasm_bindgen::from_value(change_info), serde_wasm_bindgen::from_value(tab)) {
					callback(id as u32, ci, t);
				}
			}) as Box<dyn FnMut(JsValue, JsValue, JsValue)>),
		)
	}
}
