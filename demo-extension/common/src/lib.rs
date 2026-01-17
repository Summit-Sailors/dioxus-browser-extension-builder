use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "server")]
use dioxus::prelude::*;

#[derive(Serialize, Deserialize, Debug, Error, Clone, PartialEq)]
pub enum AppError {
	#[error("Configuration is missing. Please set your Server URL and Auth Token in the extension options.")]
	MissingConfiguration,
	#[error("Could not connect to the summarization server. Please check the URL in options.")]
	Network,
	#[error("The server rejected the request: {0}")]
	ServerError(String),
	#[error("Could not find any main content on this page to summarize.")]
	NoContent,
	#[error("The content script failed to respond. Please try reloading the page.")]
	ContentScriptError,
	#[error("An internal extension error occurred: {0}")]
	ExtensionError(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
	pub server_url: String,
	pub auth_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ExtMessage {
	SummarizeRequest,
	SummarizeResponse(String),
	GetPageContent,
	Error(AppError),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerSummarizeRequest {
	pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerSummarizeResponse {
	pub summary: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerErrorResponse {
	pub error: String,
}

#[cfg(feature = "server")]
#[server(endpoint = "/api/summarize")]
pub async fn summarize(req: ServerSummarizeRequest) -> Result<ServerSummarizeResponse, ServerFnError> {
	dioxus::logger::tracing::info!("Received text to summarize: {:?}", req.text);
	let summary = format!(
		"This is a hardcoded summary for the text: '{}...'",
		req.text.chars().take(20).collect::<String>()
	);
	Ok(ServerSummarizeResponse { summary })
}
