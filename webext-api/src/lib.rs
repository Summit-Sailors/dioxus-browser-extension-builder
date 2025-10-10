pub mod api;
pub mod error;
pub mod types;
mod utils;

use api::*;
use error::ExtensionError;
use js_sys::Object;
pub use types::*;
use wasm_bindgen::prelude::*;

#[derive(Clone)]
pub struct Browser {
	api_root: Object,
	browser_type: BrowserType,
}

impl Browser {
	pub fn browser_type(&self) -> BrowserType {
		self.browser_type.clone()
	}

	pub fn action(&self) -> Action {
		Action::new(&self.api_root, self.browser_type.clone())
	}

	pub fn alarms(&self) -> Alarms {
		Alarms::new(&self.api_root)
	}

	pub fn commands(&self) -> Commands {
		Commands::new(&self.api_root)
	}

	pub fn context_menus(&self) -> ContextMenus {
		ContextMenus::new(&self.api_root)
	}

	pub fn runtime(&self) -> Runtime {
		Runtime::new(&self.api_root)
	}

	pub fn scripting(&self) -> Scripting {
		Scripting::new(&self.api_root)
	}

	pub fn storage(&self) -> Storage {
		Storage::new(&self.api_root)
	}

	pub fn tabs(&self) -> Tabs {
		Tabs::new(&self.api_root)
	}

	pub fn side_panel(&self) -> SidePanel {
		SidePanel::new(&self.api_root, self.browser_type.clone())
	}

	#[cfg(feature = "chrome")]
	pub fn declarative_net_request(&self) -> DeclarativeNetRequest {
		DeclarativeNetRequest::new(&self.api_root, self.browser_type.clone())
	}
}

pub fn init() -> Result<Browser, ExtensionError> {
	let window = web_sys::window().ok_or(ExtensionError::ApiNotFound("window".into()))?;

	if let Ok(api_root) = js_sys::Reflect::get(&window, &"chrome".into()).and_then(|v| v.dyn_into::<Object>()) {
		Ok(Browser { api_root, browser_type: BrowserType::Chrome })
	} else if let Ok(api_root) = js_sys::Reflect::get(&window, &"browser".into()).and_then(|v| v.dyn_into::<Object>()) {
		Ok(Browser { api_root, browser_type: BrowserType::Firefox })
	} else {
		Err(ExtensionError::UnsupportedBrowser)
	}
}
