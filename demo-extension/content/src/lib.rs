use {
	anyhow::anyhow,
	common::{ContentMessage, Message},
	url::Url,
	wasm_bindgen::prelude::*,
	wasm_bindgen_futures::spawn_local,
	web_sys::{window, Document, Element, HtmlElement, NodeList, Window},
};

#[wasm_bindgen(start)]
pub fn init_content() -> Result<(), JsValue> {
	console_error_panic_hook::set_once();

	spawn_local(async {
		if let Err(e) = run_content().await {
			web_sys::console::error_1(&format!("Content script error: {:?}", e).into());
		}
	});

	Ok(())
}

async fn run_content() -> Result<(), JsValue> {
	let window: Window = window().expect("no global `window` exists");
	let document = window.document().expect("should have a document on window");
	let location = document.location().expect("document should have a location");
	let url = location.href()?;

	send_message(Message::Content(ContentMessage::PageLoaded(url))).await?;

	let div_elements = document.query_selector_all("div")?;
	let div_count = div_elements.length();

	send_message(Message::Content(ContentMessage::ElementFound { selector: "div".to_string(), count: div_count as usize })).await?;

	Ok(())
}

async fn send_message(message: Message) -> Result<(), JsValue> {
	let message_js = serde_wasm_bindgen::to_value(&message)?;

	let window: Window = window().expect("no global `window` exists");
	let browser = js_sys::Reflect::get(&window, &JsValue::from_str("chrome"))?;
	let runtime = js_sys::Reflect::get(&browser, &JsValue::from_str("runtime"))?;
	let send_message = js_sys::Reflect::get(&runtime, &JsValue::from_str("sendMessage"))?;

	js_sys::Reflect::apply(&send_message.dyn_into::<js_sys::Function>()?, &runtime, &js_sys::Array::of1(&message_js))?;

	Ok(())
}

#[wasm_bindgen]
pub fn extract(mode: String) -> Result<String, JsValue> {
	web_sys::console::log_1(&format!("Extraction mode: {:?}", mode).into());
	extract_internal(mode).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn extract_internal(mode: String) -> Result<String, anyhow::Error> {
	web_sys::console::log_1(&JsValue::from(mode.clone()));
	match mode.as_str() {
		"Readability" => Ok(extract_with_readability()?),
		"Basic" => Ok(extract_basic()?),
		_ => Err(anyhow::anyhow!("Invalid extraction mode: {}", mode)),
	}
}

fn extract_with_readability() -> Result<String, anyhow::Error> {
	let window = window().ok_or(anyhow!("No window found"))?;
	let document = window.document().ok_or(anyhow!("No document found"))?;
	let location = window.location();
	let url = location.href().map_err(|_| anyhow!("Failed to get URL"))?;
	let url = Url::parse(&url)?;

	let html = document.document_element().ok_or(anyhow!("No document element found"))?.outer_html();

	let article = readability::extract(&mut html.as_bytes(), &url, Default::default())?;

	Ok(clean_content(&article.content))
}

fn extract_basic() -> Result<String, anyhow::Error> {
	let window = window().ok_or(anyhow!("No window found"))?;
	let document = window.document().ok_or(anyhow!("No document found"))?;

	let body = document.body().ok_or(anyhow!("No body found"))?;
	let noise_selectors = [
		"script",
		"style",
		"noscript",
		"iframe",
		"header",
		"footer",
		"nav",
		"aside",
		".sidebar",
		"#sidebar",
		".ad",
		".ads",
		".advertisement",
		".cookie-banner",
		"#cookie-banner",
		".social-share",
		".share-buttons",
		"#comments",
		".comments-section",
	];

	let cloned_body = body.clone_node_with_deep(true).map_err(|_| anyhow!("Cloning error"))?;
	let cloned_body = cloned_body.dyn_into::<HtmlElement>().map_err(|_| anyhow!("Cast error"))?;

	for selector in &noise_selectors {
		remove_elements(&document, &cloned_body, selector);
	}

	Ok(clean_content(&cloned_body.text_content().unwrap_or_default()))
}

fn remove_elements(document: &Document, parent: &Element, selector: &str) {
	if let Ok(elements) = document.query_selector_all(selector) {
		remove_nodes(parent, elements);
	}
}

fn remove_nodes(parent: &Element, nodes: NodeList) {
	for i in 0..nodes.length() {
		if let Some(node) = nodes.get(i) {
			let _ = parent.remove_child(&node);
		}
	}
}

fn clean_content(content: &str) -> String {
	content.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}
