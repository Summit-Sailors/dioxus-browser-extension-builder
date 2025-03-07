use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
	Popup(PopupMessage),
	Content(ContentMessage),
	Background(BackgroundMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PopupMessage {
	ButtonClicked(String),
	InputChanged(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentMessage {
	PageLoaded(String),
	ElementFound { selector: String, count: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackgroundMessage {
	ProcessComplete(String),
	Error(String),
}
