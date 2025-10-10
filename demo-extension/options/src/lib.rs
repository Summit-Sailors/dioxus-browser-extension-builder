use common::Config;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn main() {
	dioxus::logger::init(dioxus::logger::tracing::Level::DEBUG).expect("dioxus logger");
	dioxus::launch(App);
}

#[component]
fn App() -> Element {
	let mut server_url = use_signal(String::new);
	let mut auth_token = use_signal(String::new);
	let mut status_message = use_signal(String::new);

	use_effect(move || {
		spawn(async move {
			if let Ok(browser) = webext_api::init()
				&& let Ok(Some(config)) = browser.storage().local().get::<Config>("config").await
			{
				server_url.set(config.server_url);
				auth_token.set(config.auth_token);
			}
		});
	});

	let on_save = move |_| async move {
		let config = Config { server_url: server_url(), auth_token: auth_token() };
		if let Ok(browser) = webext_api::init() {
			match browser.storage().local().set("config", &config).await {
				Ok(_) => {
					status_message.set("Settings saved successfully!".to_string());
					TimeoutFuture::new(3_000).await;
					status_message.set("".to_string());
				},
				Err(e) => status_message.set(format!("Error saving settings: {}", e)),
			}
		}
	};

	rsx! {
		div { class: "max-w-md mx-auto mt-10 p-6 bg-white rounded-lg shadow-md",
			h1 { class: "text-2xl font-bold text-gray-800 mb-6", "Summarizer Options" }
			div { class: "mb-4",
				label {
					class: "block text-sm font-medium text-gray-700 mb-1",
					r#for: "server_url",
					"Server URL"
				}
				input {
					class: "w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500",
					id: "server_url",
					r#type: "text",
					placeholder: "http://localhost:3001",
					value: "{server_url}",
					oninput: move |evt| server_url.set(evt.value()),
				}
			}
			div { class: "mb-6",
				label {
					class: "block text-sm font-medium text-gray-700 mb-1",
					r#for: "auth_token",
					"Authentication Token"
				}
				input {
					class: "w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500",
					id: "auth_token",
					r#type: "password",
					placeholder: "Your secret auth token",
					value: "{auth_token}",
					oninput: move |evt| auth_token.set(evt.value()),
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
