use crate::{
	error::ExtensionError,
	utils::{call_async_fn, get_api_namespace},
};
use js_sys::{Function, Object, Reflect};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};

#[derive(Clone)]
pub struct Scripting {
	api: Object,
}

impl Scripting {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "scripting").expect("`scripting` API not available");
		Self { api }
	}

	pub async fn execute_script<T: DeserializeOwned>(&self, tab_id: u32, func: &str) -> Result<T, ExtensionError> {
		let config = Object::new();
		let target = Object::new();
		Reflect::set(&target, &"tabId".into(), &tab_id.into())?;
		Reflect::set(&config, &"target".into(), &target)?;
		Reflect::set(&config, &"func".into(), &Function::new_no_args(func))?;
		let results = call_async_fn(&self.api, "executeScript", &[config.into()][..]).await?;
		let results_array: js_sys::Array = results.dyn_into()?;
		if let Some(result_obj) = results_array.iter().next() {
			serde_wasm_bindgen::from_value(Reflect::get(&result_obj, &"result".into())?).map_err(Into::into)
		} else {
			serde_wasm_bindgen::from_value(JsValue::NULL).map_err(Into::into)
		}
	}
}
