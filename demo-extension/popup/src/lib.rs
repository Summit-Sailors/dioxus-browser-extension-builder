use common::{AppError, ToBackground, ToPopup};
use dioxus::{
	prelude::*,
	web::{Config, launch::launch_cfg},
};
use wasm_bindgen::prelude::*;

#[derive(Clone, PartialEq)]
enum AppState {
	Idle,
	Loading,
	Success(String),
	Error(AppError),
}

#[wasm_bindgen]
pub fn main() {
	dioxus::logger::init(dioxus::logger::tracing::Level::DEBUG).expect("dioxus logger");
	launch_cfg(App, Config::default());
}

#[component]
fn App() -> Element {
	let mut app_state = use_signal(|| AppState::Idle);

	use_effect(move || {
		let browser = match webext_api::init() {
			Ok(b) => b,
			Err(e) => {
				app_state.set(AppState::Error(AppError::ExtensionError(e.to_string())));
				return;
			},
		};
		let listener = match browser.runtime().on_message::<ToPopup>() {
			Ok(l) => l,
			Err(e) => {
				app_state.set(AppState::Error(AppError::ExtensionError(e.to_string())));
				return;
			},
		};
		if listener
			.add_listener(move |msg, _| match msg {
				ToPopup::SummarizeResponse(s) => app_state.set(AppState::Success(s)),
				ToPopup::Error(e) => app_state.set(AppState::Error(e)),
			})
			.is_err()
		{
			app_state.set(AppState::Error(AppError::ExtensionError("Could not attach popup listener.".to_string())));
		}
	});

	let on_summarize_click = move |_| async move {
		app_state.set(AppState::Loading);
		if let Ok(browser) = webext_api::init()
			&& let Err(e) = browser.runtime().send_message::<_, ()>(&ToBackground::SummarizeRequest).await
		{
			app_state.set(AppState::Error(AppError::ExtensionError(e.to_string())));
		}
	};

	let is_loading = matches!(app_state(), AppState::Loading);

	rsx! {
		div { class: "w-250 h-250 p-4 bg-white",
			h1 { class: "text-lg font-bold text-center text-gray-800 mb-4", "AI Page Summarizer" }
			button {
				class: "w-full px-4 py-2 text-white font-semibold rounded-md shadow-sm transition-colors duration-200 ease-in-out bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed",
				disabled: is_loading,
				onclick: on_summarize_click,
				if is_loading {
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
							SummaryView { summary: summary.clone() }
						},
						AppState::Error(error) => rsx! {
							p { class: "text-red-600 font-medium", "{error}" }
							if error == AppError::MissingConfiguration {
								p { class: "mt-2 text-sm text-gray-600",
									"You can set them in the "
									button {
										class: "text-blue-600 hover:underline font-semibold bg-transparent border-none p-0 cursor-pointer",
										onclick: move |_| {
												spawn(async {
														if let Ok(browser) = webext_api::init() {
																let _ = browser.runtime().open_options_page().await;
														}
												});
										},
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
							if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard())
							{
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
