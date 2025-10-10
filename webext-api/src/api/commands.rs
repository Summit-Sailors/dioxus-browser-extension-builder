use crate::{
	error::ExtensionError,
	types::{Command, ListenerHandle, attach_listener},
	utils::{call_async_fn_and_de, get_api_namespace},
};
use js_sys::Object;
use wasm_bindgen::{JsValue, prelude::*};

#[derive(Clone)]
pub struct Commands {
	api: Object,
}

impl Commands {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "commands").expect("`commands` API not available");
		Self { api }
	}

	pub async fn get_all(&self) -> Result<Vec<Command>, ExtensionError> {
		call_async_fn_and_de(&self.api, "getAll", &[][..]).await
	}

	pub fn on_command(&self) -> Result<OnCommand, ExtensionError> {
		Ok(OnCommand(get_api_namespace(&self.api, "onCommand")?))
	}
}

pub struct OnCommand(Object);

impl OnCommand {
	pub fn add_listener(&self, mut callback: impl FnMut(String) + 'static) -> Result<ListenerHandle<dyn FnMut(JsValue)>, ExtensionError> {
		attach_listener(
			&self.0,
			Closure::wrap(Box::new(move |val: JsValue| {
				if let Some(command) = val.as_string() {
					callback(command);
				}
			}) as Box<dyn FnMut(JsValue)>),
		)
	}
}
