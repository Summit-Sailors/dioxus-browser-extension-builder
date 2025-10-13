use common::{ServerSummarizeRequest, ServerSummarizeResponse};
use dioxus::prelude::*;

#[post("/api/summarize")]
pub async fn summarize(req: ServerSummarizeRequest) -> Result<ServerSummarizeResponse, ServerFnError> {
	dioxus::logger::tracing::info!("Received text to summarize: {:?}", req.text);
	let summary = format!("This is a hardcoded summary for the text: '{}...'", req.text.chars().take(20).collect::<String>());
	Ok(ServerSummarizeResponse { summary })
}
