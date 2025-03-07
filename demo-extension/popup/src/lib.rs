#![allow(non_snake_case)]
mod content_extractor;
use {
  content_extractor::{extract, ExtractionMode}, 
  dioxus::prelude::*, 
  dioxus_logger::tracing, 
  dioxus_web::Config, 
  wasm_bindgen::prelude::*
};

fn App() -> Element {
  let mut extracted_content = use_signal(String::new);
  let mut error_message = use_signal(String::new);
  let mut extraction_mode = use_signal(|| ExtractionMode::Readability);
  let mut is_extracting = use_signal(|| false);

  // async function to handle extraction
  let extract_content = move |_| {
    is_extracting.set(true);
    
    spawn(async move {
      match extract(extraction_mode()).await {
        Ok(result) => {
          extracted_content.set(result);
          error_message.set(String::new());
        },
        Err(e) => {
          error_message.set(format!("Error: {}", e));
          extracted_content.set(String::new());
        },
      }
      is_extracting.set(false);
    });
  };

  rsx! {
    div { class: "p-4 w-full max-w-lg mx-auto",
      h1 { class: "text-2xl font-bold mb-4", "Content Extractor" }
      
      div { class: "mb-4",
        p { class: "text-sm text-gray-400 mb-2",
          "Extracts content from the current tab using the selected mode:"
        }
        ul { class: "list-disc pl-5 text-sm text-gray-400",
          li { 
            strong { "Readability: " }
            "Attempts to identify and extract the main article content."
          }
          li { 
            strong { "Basic: " }
            "Extracts all text after removing common noise elements."
          }
        }
      }
      
      div { class: "flex gap-4 mb-4",
        button {
          class: "px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition disabled:opacity-50 disabled:cursor-not-allowed",
          disabled: is_extracting(),
          onclick: extract_content,
          if is_extracting() {
            "Extracting..."
          } else {
            "Extract Content"
          }
        }
        
        button {
          class: "px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600 transition disabled:opacity-50 disabled:cursor-not-allowed",
          disabled: is_extracting(),
          onclick: move |_| {
            extraction_mode.set(
              match extraction_mode() {
                ExtractionMode::Readability => ExtractionMode::Basic,
                ExtractionMode::Basic => ExtractionMode::Readability,
              },
            );
          },
          "Mode: {extraction_mode()}"
        }
      }
      
      if is_extracting() {
        div { class: "p-4 mb-4 text-blue-700 bg-blue-100 rounded border border-blue-400",
          "Extracting content from the active tab..."
        }
      }
      
      if !extracted_content().is_empty() {
        div { class: "mt-4",
          label { class: "block mb-2 font-medium", "Extracted Content:" }
          textarea {
            class: "w-full h-96 p-2 border rounded font-mono text-sm",
            readonly: true,
            value: extracted_content.read().clone(),
          }
        }
      } else if !is_extracting() && error_message().is_empty() {
        div { class: "p-4 mt-4 text-gray-100 bg-gray-100 rounded border border-gray-300 text-center",
          "Click \"Extract Content\" to get the content from the current tab."
        }
      }
    }
  }
}

#[wasm_bindgen(start)]
pub fn main() {
  dioxus_logger::init(tracing::Level::INFO).expect("failed to init logger");
  dioxus_web::launch::launch_cfg(App, Config::default());
}
