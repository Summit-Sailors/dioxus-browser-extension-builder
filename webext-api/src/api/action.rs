use crate::{
	error::ExtensionError,
	types::{BadgeConfig, BrowserType},
	utils::{call_async_fn, get_api_namespace},
};

#[derive(Clone)]
pub struct Action {
	api: js_sys::Object,
}

impl Action {
	pub(crate) fn new(api_root: &js_sys::Object, browser_type: BrowserType) -> Self {
		let api = match browser_type {
			BrowserType::Firefox => get_api_namespace(api_root, "action").or_else(|_| get_api_namespace(api_root, "browserAction")),
			_ => get_api_namespace(api_root, "action"),
		}
		.expect("Could not find action API namespace");
		Self { api }
	}

	pub async fn set_badge_text(&self, config: BadgeConfig) -> Result<(), ExtensionError> {
		let details = serde_wasm_bindgen::to_value(&config)?;
		call_async_fn(&self.api, "setBadgeText", &[details.clone()][..]).await?;
		if config.background_color.is_some() {
			call_async_fn(&self.api, "setBadgeBackgroundColor", &[details][..]).await?;
		}
		Ok(())
	}

	pub async fn clear_badge(&self) -> Result<(), ExtensionError> {
		self.set_badge_text(BadgeConfig { text: Some("".to_string()), ..Default::default() }).await
	}
}
