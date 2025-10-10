use thiserror::Error;
use wasm_bindgen::{JsCast, prelude::*};

#[derive(Error, Debug)]
pub enum ExtensionError {
	#[error("The `{0}` API is not available in this context.")]
	ApiNotFound(String),

	#[error("Could not find the specified tab.")]
	TabNotFound,

	#[error("This browser is not supported or no extension API was found.")]
	UnsupportedBrowser,

	#[error("Script execution in the target tab failed.")]
	ScriptExecutionFailed,

	#[error("Failed to serialize or deserialize data: {0}")]
	SerializationError(#[from] serde_wasm_bindgen::Error),

	#[error("The browser API returned an error: {0}")]
	ApiError(String),

	#[error("A JavaScript error occurred: {message}")]
	JsError { message: String, js_value: JsValue },

	#[error("An unexpected JavaScript value was thrown: {0:?}")]
	JsValue(JsValue),
}

impl From<JsValue> for ExtensionError {
	fn from(js_val: JsValue) -> Self {
		if let Some(obj) = js_val.dyn_ref::<js_sys::Object>()
			&& let Ok(message_val) = js_sys::Reflect::get(obj, &"message".into())
			&& let Some(message) = message_val.as_string()
		{
			return ExtensionError::ApiError(message);
		}

		if let Some(e) = js_val.dyn_ref::<js_sys::Error>() {
			ExtensionError::JsError { message: e.message().into(), js_value: js_val }
		} else {
			ExtensionError::JsValue(js_val)
		}
	}
}
