use serde::{Deserialize, Serialize};
use thiserror::Error;

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
pub enum ToBackground {
	SummarizeRequest,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ToPopup {
	SummarizeResponse(String),
	Error(AppError),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ToContentScript {
	GetPageContent,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerSummarizeRequest {
	pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerSummarizeResponse {
	pub summary: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerErrorResponse {
	pub error: String,
}
