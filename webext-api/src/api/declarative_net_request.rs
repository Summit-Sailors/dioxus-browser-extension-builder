use crate::{
	error::ExtensionError,
	types::{BrowserType, UpdateRulesOptions},
	utils::{call_async_fn, get_api_namespace},
};
use js_sys::Object;
use serde_wasm_bindgen::to_value;

#[derive(Clone)]
pub struct DeclarativeNetRequest {
	api: Option<Object>,
}

impl DeclarativeNetRequest {
	pub(crate) fn new(api_root: &Object, browser_type: BrowserType) -> Self {
		let api = match browser_type {
			BrowserType::Chrome => get_api_namespace(api_root, "declarativeNetRequest").ok(),
			BrowserType::Firefox => None,
		};
		Self { api }
	}

	pub async fn update_dynamic_rules(&self, options: UpdateRulesOptions) -> Result<(), ExtensionError> {
		if let Some(api) = &self.api {
			call_async_fn(api, "updateDynamicRules", &[to_value(&options)?][..]).await?;
			Ok(())
		} else {
			Err(ExtensionError::ApiNotFound("declarativeNetRequest".to_string()))
		}
	}
}
