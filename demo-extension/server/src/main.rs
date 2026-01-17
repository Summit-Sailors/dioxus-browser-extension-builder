use common::{ServerSummarizeRequest, ServerSummarizeResponse};
use dioxus::server::axum::{Json, Router, routing::post};

#[allow(unused_imports)]
use server::*;

async fn summarize_handler(Json(req): Json<ServerSummarizeRequest>) -> Json<ServerSummarizeResponse> {
	dioxus::logger::tracing::info!("Received text to summarize: {:?}", req.text);
	let summary = format!(
		"This is a hardcoded summary for the text: '{}...'",
		req.text.chars().take(20).collect::<String>()
	);
	Json(ServerSummarizeResponse { summary })
}

fn main() {
	dioxus::logger::initialize_default();
	dioxus::serve(|| async {
		Ok::<Router, anyhow::Error>(Router::new().route("/api/summarize", post(summarize_handler)))
	});
}
