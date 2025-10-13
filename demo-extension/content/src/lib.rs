use common::ExtMessage;
use dioxus::prelude::*;
use js_sys::Function;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::{JsCast, prelude::*};
use web_extensions_sys::chrome;
use web_sys::{Element, window};

fn get_main_content() -> String {
	let document = window().expect("window").document().expect("document");
	if let Ok(tags) = document.query_selector_all("script, style, nav, header, footer, aside") {
		for i in 0..tags.length() {
			if let Some(node) = tags.item(i)
				&& let Ok(element_to_remove) = node.dyn_into::<Element>()
			{
				element_to_remove.remove();
			}
		}
	}
	document.body().expect("body").text_content().unwrap_or_default()
}

#[wasm_bindgen]
pub fn main() {
	dioxus::logger::initialize_default();

	let closure = Closure::<dyn FnMut(JsValue, JsValue, Function)>::new(|message: JsValue, _sender: JsValue, send_response: Function| {
		if let Ok(ExtMessage::GetPageContent) = from_value(message) {
			let content = get_main_content();
			match to_value(&content) {
				Ok(js_val) => {
					if let Err(e) = send_response.call1(&JsValue::UNDEFINED, &js_val) {
						error!("[content_script] Failed to send response: {:?}", e);
					}
				},
				Err(e) => error!("[content_script] Failed to serialize page content: {}", e.to_string()),
			}
		}
	});
	chrome().runtime().on_message().add_listener(closure.as_ref().unchecked_ref());
	closure.forget();
}
