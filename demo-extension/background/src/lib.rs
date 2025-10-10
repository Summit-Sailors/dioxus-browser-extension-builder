use common::{AppError, Config, ServerErrorResponse, ServerSummarizeRequest, ServerSummarizeResponse, ToBackground, ToContentScript, ToPopup};
use reqwest::{Client, header};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn main() {
	wasm_logger::init(wasm_logger::Config::default());

	let browser = match webext_api::init() {
		Ok(b) => b,
		Err(e) => {
			log::error!("[background] Failed to initialize: {}", e);
			return;
		},
	};

	let listener = match browser.runtime().on_message::<ToBackground>() {
		Ok(l) => l,
		Err(e) => {
			log::error!("[background] Failed to get listener: {}", e);
			return;
		},
	};

	if listener
		.add_listener(move |msg, _| {
			wasm_bindgen_futures::spawn_local(async move {
				let browser = match webext_api::init() {
					Ok(b) => b,
					Err(e) => {
						log::error!("[background:async] Failed to re-init: {}", e);
						return;
					},
				};
				if let ToBackground::SummarizeRequest = msg {
					let result = handle_summarize_request(&browser).await;
					let response_message = match result {
						Ok(summary) => ToPopup::SummarizeResponse(summary),
						Err(e) => ToPopup::Error(e),
					};
					if let Err(e) = browser.runtime().send_message::<_, ()>(&response_message).await {
						log::error!("[background] Failed to send response: {}", e);
					}
				}
			});
		})
		.is_err()
	{
		log::error!("[background] Failed to attach listener.");
	}
}

async fn get_config(browser: &webext_api::Browser) -> Result<Config, AppError> {
	let config: Option<Config> = browser.storage().local().get("config").await.map_err(|e| AppError::ExtensionError(e.to_string()))?;
	config.ok_or(AppError::MissingConfiguration)
}

async fn handle_summarize_request(browser: &webext_api::Browser) -> Result<String, AppError> {
	let config = get_config(browser).await?;
	if config.server_url.is_empty() || config.auth_token.is_empty() {
		return Err(AppError::MissingConfiguration);
	}

	let mut headers = header::HeaderMap::new();
	headers.insert("X-Auth-Token", header::HeaderValue::from_str(&config.auth_token).map_err(|_| AppError::ExtensionError("Invalid auth token".to_string()))?);
	let http_client = Client::builder().default_headers(headers).build().map_err(|_| AppError::ExtensionError("Failed to build client".to_string()))?;

	let tab = browser.tabs().get_active().await.map_err(|e| AppError::ExtensionError(e.to_string()))?;
	let tab_id = tab.id.ok_or(AppError::ExtensionError("Active tab has no ID".to_string()))?;
	let text: String = browser.tabs().send_message(tab_id, &ToContentScript::GetPageContent).await.map_err(|_| AppError::ContentScriptError)?;

	if text.trim().is_empty() {
		return Err(AppError::NoContent);
	}

	let res =
		http_client.post(format!("{}/api/summarize", config.server_url)).json(&ServerSummarizeRequest { text }).send().await.map_err(|_| AppError::Network)?;

	if !res.status().is_success() {
		let err_res = res.json::<ServerErrorResponse>().await.map_err(|_| AppError::ServerError("Failed to parse error.".to_string()))?;
		return Err(AppError::ServerError(err_res.error));
	}

	let summary_res = res.json::<ServerSummarizeResponse>().await.map_err(|_| AppError::ServerError("Failed to parse summary.".to_string()))?;
	Ok(summary_res.summary)
}
