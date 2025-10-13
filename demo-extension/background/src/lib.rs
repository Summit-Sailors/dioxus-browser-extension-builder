use common::ExtMessage;
use common::ServerSummarizeRequest;
use dioxus::prelude::*;
use js_sys::Function;
use server::summarize;
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
	info!("setting server url");
	dioxus::fullstack::set_server_url(SERVER_URL);
	info!("starting message listener");
	start_listener();
}

async fn handle_summarize_request() -> Result<String, ExtensionError> {
	info!("sending get content request to the content script");
	let message = serde_wasm_bindgen::to_value(&ExtMessage::GetPageContent)?;
	let response_js = web_extensions_sys::chrome().runtime().send_message(None, &message, None).await?;
	let text: String = serde_wasm_bindgen::from_value(response_js)?;
	info!("checking response is empty");
	if text.trim().is_empty() {
		return Err(ExtensionError::ApiError("text is empty".to_string()));
	}
	info!("sending content response to BE server");
	let summary_res = summarize(ServerSummarizeRequest { text }).await.map_err(|e| ExtensionError::ApiError(e.to_string()))?;
	Ok(summary_res.summary)
}
