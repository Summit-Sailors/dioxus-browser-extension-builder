use crate::{
	error::ExtensionError,
	types::{ContextMenuConfig, ListenerHandle, OnClickData, attach_listener},
	utils::{call_async_fn, get_api_namespace},
};
use js_sys::Object;
use serde_wasm_bindgen::to_value;
use wasm_bindgen::{JsValue, prelude::*};

#[derive(Clone)]
pub struct ContextMenus {
	api: Object,
}

impl ContextMenus {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "contextMenus").expect("`contextMenus` API not available");
		Self { api }
	}

	pub async fn create(&self, config: ContextMenuConfig) -> Result<(), ExtensionError> {
		call_async_fn(&self.api, "create", &[to_value(&config)?][..]).await?;
		Ok(())
	}

	pub async fn remove_all(&self) -> Result<(), ExtensionError> {
		call_async_fn(&self.api, "removeAll", &[][..]).await?;
		Ok(())
	}

	pub fn on_clicked(&self) -> Result<OnMenuClicked, ExtensionError> {
		Ok(OnMenuClicked(get_api_namespace(&self.api, "onClicked")?))
	}
}

pub struct OnMenuClicked(Object);

impl OnMenuClicked {
	pub fn add_listener(&self, mut callback: impl FnMut(OnClickData) + 'static) -> Result<ListenerHandle<dyn FnMut(JsValue)>, ExtensionError> {
		attach_listener(
			&self.0,
			Closure::wrap(Box::new(move |val: JsValue| {
				if let Ok(data) = serde_wasm_bindgen::from_value(val) {
					callback(data);
				}
			}) as Box<dyn FnMut(JsValue)>),
		)
	}
}
