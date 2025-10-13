use common::{AppError, ExtMessage};
use dioxus::{
	prelude::*,
	web::{Config, launch::launch_cfg},
};
use wasm_bindgen::prelude::*;
use web_sys::js_sys;

#[derive(Clone, PartialEq)]
enum AppState {
	Idle,
	Loading,
	Success(String),
	Error(AppError),
}

#[wasm_bindgen]
pub fn main() {
	dioxus::logger::initialize_default();
	launch_cfg(App, Config::default());
}

fn start_message_listener(mut app_state: Signal<AppState>) {
	let listener = Closure::wrap(Box::new(move |message: JsValue, _sender: web_extensions_sys::MessageSender, _send_response: js_sys::Function| {
		match serde_wasm_bindgen::from_value::<ExtMessage>(message) {
			Ok(msg) => match msg {
				ExtMessage::SummarizeResponse(s) => app_state.set(AppState::Success(s)),
				ExtMessage::Error(e) => app_state.set(AppState::Error(e)),
				_ => {},
			},
			Err(e) => {
				let err_msg = format!("Failed to deserialize message: {}", e);
				error!("{}", err_msg);
				app_state.set(AppState::Error(AppError::ExtensionError(err_msg)));
			},
		}
	}) as Box<dyn FnMut(JsValue, web_extensions_sys::MessageSender, js_sys::Function)>);
	web_extensions_sys::chrome().runtime().on_message().add_listener(listener.as_ref().unchecked_ref());
	listener.forget();
}

#[component]
fn App() -> Element {
	let mut app_state = use_signal(|| AppState::Idle);

	use_effect(move || {
		start_message_listener(app_state);
	});

	let is_loading = use_memo(move || matches!(app_state(), AppState::Loading));

	rsx! {
		div { class: "w-250 h-250 p-4 bg-white",
			h1 { class: "text-lg font-bold text-center text-gray-800 mb-4", "AI Page Summarizer" }
			button {
				class: "w-full px-4 py-2 text-white font-semibold rounded-md shadow-sm transition-colors duration-200 ease-in-out bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed",
				disabled: is_loading,
				onclick: move |_| async move {
						app_state.set(AppState::Loading);
						match serde_wasm_bindgen::to_value(&ExtMessage::SummarizeRequest) {
								Ok(message) => {
										match web_extensions_sys::chrome()
												.runtime()
												.send_message(None, &message, None)
												.await
										{
												Ok(_) => info!("SummarizeRequest message sent successfully"),
												Err(e) => {
														let error_str = e
																.as_string()
																.unwrap_or_else(|| "Unknown JavaScript error".to_string());
														error!("Error sending message: {}", error_str);
														app_state
																.set(AppState::Error(AppError::ExtensionError(error_str)));
												}
										}
								}
								Err(e) => {
										let err_msg = format!("Failed to serialize message: {}", e);
										error!("{}", err_msg);
										app_state.set(AppState::Error(AppError::ExtensionError(err_msg)));
								}
						}
				},
				if is_loading() {
					"Summarizing..."
				} else {
					"Summarize Page"
				}
			}
			div { class: "relative mt-4 p-3 bg-gray-50 border border-gray-200 rounded-md min-h-[120px] text-gray-700 text-sm leading-relaxed",
				match app_state() {
						AppState::Idle => rsx! {
							p { class: "text-gray-500", "Click the button to generate a summary." }
						},
						AppState::Loading => rsx! {
							div { class: "absolute inset-0 flex items-center justify-center",
								div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" }
							}
						},
						AppState::Success(summary) => rsx! {
							SummaryView { summary }
						},
						AppState::Error(error) => rsx! {
							p { class: "text-red-600 font-medium", "{error}" }
							if error == AppError::MissingConfiguration {
								p { class: "mt-2 text-sm text-gray-600",
									"You can set them in the "
									button {
										class: "text-blue-600 hover:underline font-semibold bg-transparent border-none p-0 cursor-pointer",
										onclick: move |_| web_extensions_sys::chrome().runtime().open_options_page(),
										"extension options."
									}
								}
							}
						},
				}
			}
		}
	}
}

#[component]
fn SummaryView(summary: String) -> Element {
	let mut copy_text = use_signal(|| "Copy".to_string());
	rsx! {
		p { "{summary}" }
		button {
			class: "absolute top-2 right-2 px-2 py-1 text-xs font-medium text-gray-600 bg-gray-200 hover:bg-gray-300 rounded-md transition-all",
			onclick: move |_| {
					to_owned![summary];
					async move {
							if let Some(window) = web_sys::window() {
									let clipboard = window.navigator().clipboard();
									if wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&summary))
											.await
											.is_ok()
									{
											copy_text.set("Copied!".to_owned());
									} else {
											copy_text.set("Failed".to_owned());
									}
							}
					}
			},
			"{copy_text}"
		}
	}
}
