use common::{ExtMessage, ServerSummarizeRequest, ServerSummarizeResponse};
use dioxus::prelude::*;
use js_sys::Function;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_extensions_sys::chrome;
use webext_api::error::ExtensionError;

async fn listener() -> Result<(), ExtensionError> {
	info!("handling summary call");
	let summary = handle_summarize_request().await?;
	info!("sending response back to the popup");
	let message = serde_wasm_bindgen::to_value(&ExtMessage::SummarizeResponse(summary))?;
	chrome().runtime().send_message(None, &message, None).await?;
	Ok(())
}

fn start_listener() {
	let closure = Closure::<dyn FnMut(JsValue, JsValue, Function)>::new(|message: JsValue, _sender: JsValue, _send_response: Function| {
		if let Ok(ExtMessage::SummarizeRequest) = serde_wasm_bindgen::from_value(message) {
			info!("spawning wasm local async fn");
			wasm_bindgen_futures::spawn_local(async move {
				info!("starting actual listener");
				if let Err(e) = listener().await {
					error!("{}", e.to_string());
				}
			});
		}
	});
	chrome().runtime().on_message().add_listener(closure.as_ref().unchecked_ref());
	closure.forget();
}

const SERVER_URL: &str = env!("SERVER_URL");

#[wasm_bindgen]
pub fn main() {
	dioxus::logger::initialize_default();
	info!("background script initialized with server URL: {}", SERVER_URL);
	start_listener();
}

async fn call_summarize_api(req: ServerSummarizeRequest) -> Result<ServerSummarizeResponse, ExtensionError> {
	let url = format!("{}/api/summarize", SERVER_URL);
	let client = reqwest::Client::new();
	let response = client
		.post(&url)
		.json(&req)
		.send()
		.await
		.map_err(|e| ExtensionError::ApiError(format!("Request failed: {}", e)))?;

	if !response.status().is_success() {
		let status = response.status();
		let body = response.text().await.unwrap_or_default();
		return Err(ExtensionError::ApiError(format!("Server error {}: {}", status, body)));
	}

	response
		.json::<ServerSummarizeResponse>()
		.await
		.map_err(|e| ExtensionError::ApiError(format!("Failed to parse response: {}", e)))
}

async fn handle_summarize_request() -> Result<String, ExtensionError> {
	info!("sending get content request to the content script");
	let browser = webext_api::init()?;
	let tab = browser.tabs().get_active().await?;
	let tab_id = tab.id.ok_or_else(|| ExtensionError::ApiError("No tab id".to_string()))?;
	info!("sending to tab {}", tab_id);
	let text: String = browser.tabs().send_message(tab_id, &ExtMessage::GetPageContent).await?;
	info!("checking response is empty");
	if text.trim().is_empty() {
		return Err(ExtensionError::ApiError("text is empty".to_string()));
	}
	info!("sending content to server at {}", SERVER_URL);
	let summary_res = call_summarize_api(ServerSummarizeRequest { text }).await?;
	Ok(summary_res.summary)
}
