use {
	common::{BackgroundMessage, ContentMessage, Message, PopupMessage},
	wasm_bindgen::prelude::*,
	wasm_bindgen_futures::spawn_local,
};

#[wasm_bindgen(start)]
pub fn init_background() -> Result<(), JsValue> {
  console_error_panic_hook::set_once();

  let global = js_sys::global();

  let chrome = js_sys::Reflect::get(&global, &JsValue::from_str("chrome")).expect("Expected 'chrome' property in global object");
  let runtime = js_sys::Reflect::get(&chrome, &JsValue::from_str("runtime")).expect("Expected 'runtime' property on chrome object");
  let on_message = js_sys::Reflect::get(&runtime, &JsValue::from_str("onMessage")).expect("Expected 'onMessage' property on runtime");
  let add_listener = js_sys::Reflect::get(&on_message, &JsValue::from_str("addListener")).expect("Expected 'addListener' property on onMessage");

  let closure = Closure::wrap(Box::new(move |message: JsValue| {
    spawn_local(async move {
      handle_message(message).await;
    });
  }) as Box<dyn FnMut(JsValue)>);

  js_sys::Reflect::apply(
    &add_listener.dyn_into::<js_sys::Function>()?,
    &on_message,
    &js_sys::Array::of1(closure.as_ref().unchecked_ref())
  )?;

  closure.forget();

  Ok(())
}


async fn handle_message(event: JsValue) {
	if let Ok(message) = serde_wasm_bindgen::from_value::<Message>(event) {
		match message {
			Message::Popup(popup_msg) => handle_popup_message(popup_msg).await,
			Message::Content(content_msg) => handle_content_message(content_msg).await,
			Message::Background(_) => {},
		}
	}
}

async fn handle_popup_message(message: PopupMessage) {
	match message {
		PopupMessage::ButtonClicked(value) => {
			web_sys::console::log_1(&format!("Button clicked with value: {}", value).into());
			let response = Message::Background(BackgroundMessage::ProcessComplete(format!("Processed button click: {}", value)));
			send_message(response).await;
		},
		PopupMessage::InputChanged(value) => {
			web_sys::console::log_1(&format!("Input changed: {}", value).into());
		},
	}
}

async fn handle_content_message(message: ContentMessage) {
	match message {
		ContentMessage::PageLoaded(url) => {
			web_sys::console::log_1(&format!("Page loaded: {}", url).into());
		},
		ContentMessage::ElementFound { selector, count } => {
			web_sys::console::log_1(&format!("Found {} elements with selector: {}", count, selector).into());
		},
	}
}

async fn send_message(message: Message) {
	let message_js = serde_wasm_bindgen::to_value(&message).unwrap();

	let global = js_sys::global();
	let browser = js_sys::Reflect::get(&global, &JsValue::from_str("chrome")).unwrap();
	let runtime = js_sys::Reflect::get(&browser, &JsValue::from_str("runtime")).unwrap();
	let send_message = js_sys::Reflect::get(&runtime, &JsValue::from_str("sendMessage")).unwrap();

	let _ = js_sys::Reflect::apply(&send_message.dyn_into::<js_sys::Function>().unwrap(), &runtime, &js_sys::Array::of1(&message_js));
}
