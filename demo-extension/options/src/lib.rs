use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn main() {
	dioxus::logger::initialize_default();
	dioxus::launch(App);
}

#[component]
fn App() -> Element {
	let mut enable_notifications = use_signal(|| true);
	let mut summary_style = use_signal(|| "bullets".to_string());
	let mut status_message = use_signal(String::new);

	let on_save = move |_| async move {
		status_message.set("Settings saved successfully!".to_string());
		TimeoutFuture::new(2_000).await;
		status_message.set("".to_string());
	};

	rsx! {
		div { class: "max-w-md mx-auto mt-10 p-6 bg-white rounded-lg shadow-md font-sans",
			h1 { class: "text-2xl font-bold text-gray-800 mb-6", "Extension Settings" }

			div { class: "flex items-center justify-between mb-4 py-2",
				label {
					class: "text-base font-medium text-gray-700",
					r#for: "enable_notifications",
					"Enable Notifications"
				}
				label { class: "relative inline-flex items-center cursor-pointer",
					input {
						class: "sr-only peer",
						id: "enable_notifications",
						r#type: "checkbox",
						checked: enable_notifications,
						oninput: move |evt| enable_notifications.set(evt.value() == "true"),
					}
					div { class: "w-11 h-6 bg-gray-200 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600" }
				}
			}

			div { class: "mb-6 py-2",
				label {
					class: "block text-base font-medium text-gray-700 mb-2",
					r#for: "summary_style",
					"Summarization Style"
				}
				select {
					class: "w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500",
					id: "summary_style",
					onchange: move |evt| summary_style.set(evt.value()),
					option {
						value: "bullets",
						selected: summary_style() == "bullets",
						"Bullet Points"
					}
					option {
						value: "paragraph",
						selected: summary_style() == "paragraph",
						"Single Paragraph"
					}
				}
			}

			button {
				class: "w-full px-4 py-2 text-white font-semibold rounded-md shadow-sm transition-colors duration-200 ease-in-out bg-blue-600 hover:bg-blue-700",
				onclick: on_save,
				"Save Settings"
			}

			if !status_message().is_empty() {
				p { class: "mt-4 text-sm text-center text-green-600", "{status_message}" }
			}
		}
	}
}
