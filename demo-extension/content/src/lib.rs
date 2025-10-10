use common::ToContentScript;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Element, window};

fn get_main_content() -> String {
	let document = match window().and_then(|w| w.document()) {
		Some(doc) => doc,
		None => return "".to_string(),
	};
	let selectors = ["main", "article", "div[role='main']"];
	for selector in selectors {
		if let Ok(Some(element)) = document.query_selector(selector) {
			let clone = element.clone();
			if let Ok(tags) = clone.query_selector_all("script, style, nav, header, footer, aside") {
				for i in 0..tags.length() {
					if let Some(node) = tags.item(i)
						&& let Ok(element_to_remove) = node.dyn_into::<Element>()
					{
						element_to_remove.remove();
					}
				}
			}
			return clone.text_content().unwrap_or_default();
		}
	}
	document.body().map(|b| b.text_content().unwrap_or_default()).unwrap_or_default()
}

#[wasm_bindgen]
pub fn main() {
	wasm_logger::init(wasm_logger::Config::new(log::Level::Info));
	match webext_api::init() {
		Ok(browser) => match browser.runtime().on_message::<ToContentScript>() {
			Ok(on_message) => {
				if on_message
					.add_listener_with_response(move |msg, _| async move {
						match msg {
							ToContentScript::GetPageContent => Ok(get_main_content()),
						}
					})
					.is_err()
				{
					log::error!("[content_script] Could not attach listener.");
				}
			},
			Err(e) => log::error!("[content_script] Could not get on_message handle: {}", e),
		},
		Err(e) => log::error!("[content_script] Could not initialize: {}", e),
	}
}
