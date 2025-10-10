use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client as OpenAIClient,
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use common::{ServerErrorResponse, ServerSummarizeRequest, ServerSummarizeResponse};
use std::{env, net::SocketAddr};
use thiserror::Error;
use tracing::info;

#[derive(Clone)]
struct AppState {
    openai_client: OpenAIClient,
    auth_token: String,
    openai_model: String,
    openai_system_prompt: String,
}

#[derive(Debug, Error)]
enum ServerAppError {
    #[error("OpenAI API error: {0}")]
    OpenAIError(#[from] async_openai::error::OpenAIError),
    #[error("Failed to generate summary")]
    SummaryGenerationError,
    #[error("Authentication failed")]
    AuthError,
}

impl IntoResponse for ServerAppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServerAppError::OpenAIError(e) => (StatusCode::BAD_GATEWAY, e.to_string()),
            ServerAppError::SummaryGenerationError => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ServerAppError::AuthError => (StatusCode::UNAUTHORIZED, self.to_string()),
        };
        (status, Json(ServerErrorResponse { error: error_message })).into_response()
    }
}

async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ServerAppError> {
    if let Some(token) = headers.get("x-auth-token").and_then(|h| h.to_str().ok()) {
        if token == state.auth_token {
            return Ok(next.run(request).await);
        }
    }
    Err(ServerAppError::AuthError)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    dotenvy::dotenv().expect("Failed to read .env file");
    env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    let state = AppState {
        openai_client: OpenAIClient::new(),
        auth_token: env::var("AUTH_TOKEN").expect("AUTH_TOKEN must be set"),
        openai_model: env::var("OPENAI_MODEL").expect("OPENAI_MODEL must be set"),
        openai_system_prompt: env::var("OPENAI_SYSTEM_PROMPT").expect("OPENAI_SYSTEM_PROMPT must be set"),
    };
    
    let port: u16 = env::var("SERVER_PORT").unwrap_or_else(|_| "3001".to_string()).parse().expect("SERVER_PORT must be a number");

    let app = Router::new()
        .route("/api/summarize", post(summarize_handler))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn summarize_handler(
    State(state): State<AppState>,
    Json(payload): Json<ServerSummarizeRequest>,
) -> Result<Json<ServerSummarizeResponse>, ServerAppError> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(state.openai_model)
        .max_tokens(200u16)
        .messages([
            ChatCompletionRequestMessageArgs::default().role(Role::System).content(state.openai_system_prompt).build()?,
            ChatCompletionRequestMessageArgs::default().role(Role::User).content(payload.text).build()?,
        ])
        .build()?;

    let response = state.openai_client.chat().create(request).await?;
    let summary = response.choices.get(0).and_then(|c| c.message.content.as_ref()).ok_or(ServerAppError::SummaryGenerationError)?.clone();

    Ok(Json(ServerSummarizeResponse { summary }))
}