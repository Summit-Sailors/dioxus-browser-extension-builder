use crate::{
	error::ExtensionError,
	types::BrowserType,
	utils::{call_async_fn, get_api_namespace},
};
use js_sys::Object;

#[derive(Clone)]
pub struct SidePanel {
	api_root: Object,
	browser_type: BrowserType,
}

impl SidePanel {
	pub(crate) fn new(api_root: &Object, browser_type: BrowserType) -> Self {
		Self { api_root: api_root.clone(), browser_type }
	}

	pub async fn open(&self, tab_id: Option<u32>) -> Result<(), ExtensionError> {
		match self.browser_type {
			BrowserType::Chrome => {
				let side_panel_api = get_api_namespace(&self.api_root, "sidePanel")?;
				let options = Object::new();
				if let Some(id) = tab_id {
					js_sys::Reflect::set(&options, &"tabId".into(), &id.into())?;
				}
				call_async_fn(&side_panel_api, "open", &[options.into()][..]).await?;
				Ok(())
			},
			BrowserType::Firefox => {
				let sidebar_action_api = get_api_namespace(&self.api_root, "sidebarAction")?;
				call_async_fn(&sidebar_action_api, "open", &[][..]).await?;
				Ok(())
			},
		}
	}
}
