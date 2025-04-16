use {
	crate::{
		App,
		common::{BuilState, BuildMode, BuildStatus, ExtConfig, InitOptions, TomlConfig},
	},
	anyhow::{Context, Result},
	dialoguer::{Confirm, Input},
	std::{fs, io::Write, path::Path, sync::Arc},
	tokio::sync::Mutex,
	tracing::info,
};

pub(crate) fn read_config() -> Result<ExtConfig> {
	let toml_content = fs::read_to_string("dx-ext.toml").context("Failed to read dx-ext.toml file")?;

	let parsed_toml: TomlConfig = toml::from_str(&toml_content).context("Failed to parse dx-ext.toml file")?;

	// converting to our internal config structure
	Ok(ExtConfig {
		background_script_index_name: parsed_toml.extension_config.background_script_index_name,
		content_script_index_name: parsed_toml.extension_config.content_script_index_name,
		extension_directory_name: parsed_toml.extension_config.extension_directory_name,
		popup_name: parsed_toml.extension_config.popup_name,
		assets_dir: parsed_toml.extension_config.assets_directory,
		build_mode: BuildMode::Development,
		enable_incremental_builds: parsed_toml.extension_config.enable_incremental_builds,
	})
}

pub(crate) fn create_default_config_toml(options: &InitOptions) -> Result<bool> {
	info!("Welcome to the Dioxus Browser Extension Builder Setup");

	if Path::new("dx-ext.toml").exists() && !options.force {
		info!("Config file already exists. Use --force to overwrite.");
		return Ok(false);
	}

	let get_interactive_or_default = |prompt: &str, default: &str| -> Result<String> {
		if options.interactive { Ok(Input::new().with_prompt(prompt).default(default.to_owned()).interact_text()?) } else { Ok(default.to_owned()) }
	};

	let get_interactive_bool_or_default = |prompt: &str, default: bool| -> Result<bool> {
		if options.interactive { Ok(Confirm::new().with_prompt(prompt).default(default).interact()?) } else { Ok(default) }
	};

	// Use the helper functions to simplify value retrieval
	let extension_dir = get_interactive_or_default("Enter extension directory name", &options.extension_dir)?;

	let popup_name = get_interactive_or_default("Enter popup crate name", &options.popup_name)?;

	let background_script = get_interactive_or_default("Enter background script entry point", &options.background_script)?;

	let content_script = get_interactive_or_default("Enter content script entry point", &options.content_script)?;

	let enable_incremental_builds = get_interactive_bool_or_default("Enable incremental builds?", options.enable_incremental_builds)?;

	let assets_dir = get_interactive_or_default("Enter assets directory", format!("{}/assets", &popup_name).as_str())?;

	let config_content = format!(
		r#"[extension-config]
assets-directory = "{assets_dir}"
background-script-index-name = "{background_script}"
content-script-index-name = "{content_script}"
extension-directory-name = "{extension_dir}"
popup-name = "{popup_name}"
enable-incremental-builds = {enable_incremental_builds}
"#
	);

	fs::write("dx-ext.toml", config_content).context("Failed to write dx-ext.toml file")?;

	info!("Configuration created successfully:");
	info!(" Extension directory: {extension_dir}");
	info!(" Popup crate: {popup_name}");
	info!(" Background script: {background_script}");
	info!(" Content script: {content_script}");
	info!(" Assets directory: {assets_dir}");
	info!(" Enable incremental builds: {}", enable_incremental_builds);

	Ok(true)
}

pub(crate) fn generate_project_structure(config: &ExtConfig) -> Result<()> {
	if !Path::new(&config.extension_directory_name).exists() {
		let _ = fs::create_dir_all(&config.extension_directory_name).context("Failed to create extension directory");
	}

	// directory paths
	let background_dir = format!("{}/background", config.extension_directory_name);
	let background_src_dir = format!("{background_dir}/src");
	let content_dir = format!("{}/content", config.extension_directory_name);
	let content_src_dir = format!("{content_dir}/src");
	let popup_dir = format!("{}/{}", config.extension_directory_name, config.popup_name);
	let popup_src_dir = format!("{popup_dir}/src");
	let assets_dir = format!("{popup_dir}/assets");

	// create all
	fs::create_dir_all(&background_src_dir).expect("Failed to create background source directory");
	fs::create_dir_all(&content_src_dir).expect("Failed to create background source directory");
	fs::create_dir_all(&popup_src_dir).expect("Failed to create background source directory");
	fs::create_dir_all(&assets_dir).expect("Failed to create background source directory");

	// background script files
	create_cargo_toml(&background_dir, "background")?;
	create_lib_rs(&background_src_dir, "Background Script")?;
	create_js_entry_point(&config.extension_directory_name, &config.background_script_index_name, "background")?;

	// content script files
	create_cargo_toml(&content_dir, "content")?;
	create_lib_rs(&content_src_dir, "Content Script")?;
	create_js_entry_point(&config.extension_directory_name, &config.content_script_index_name, "content")?;

	// popup files
	create_cargo_toml(&popup_dir, &config.popup_name)?;
	create_lib_rs(&popup_src_dir, "Popup UI")?;
	create_html_file(&config.extension_directory_name)?;
	create_js_entry_point(&config.extension_directory_name, "index.js", "popup")?;

	// manifest.json
	create_manifest_json(&config.extension_directory_name)?;

	info!("Project structure generated successfully");

	Ok(())
}

fn create_cargo_toml(dir_path: &str, crate_name: &str) -> Result<()> {
	let cargo_content = format!(
		r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2.100"
wasm-bindgen-futures = "0.4.50"
console_error_panic_hook = "0.1.7"
gloo-utils = "0.2.0"
js-sys = "0.3.77"
serde-wasm-bindgen = "0.6.5"
web-sys = {{ version = "0.3.77", features = ["Document", "Element", "EventTarget", "Location", "NodeList", "Window", "console"] }}
"#
	);

	let cargo_path = format!("{dir_path}/Cargo.toml");
	let mut file = fs::File::create(&cargo_path).context(format!("Failed to create Cargo.toml in {dir_path}"))?;
	file.write_all(cargo_content.as_bytes()).context("Failed to write to Cargo.toml")?;
	Ok(())
}

fn create_lib_rs(dir_path: &str, component_name: &str) -> Result<()> {
	let lib_content = format!(
		r#"use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn initialize() {{
  // {component_name} initialization code
  console_log!("Initialized {component_name} successfully");
}}

#[wasm_bindgen]
extern "C" {{
  #[wasm_bindgen(js_namespace = console)]
  fn log(s: &str);
}}

#[macro_export]
macro_rules! console_log {{
  ($($t:tt)*) => (log(&format!($($t)*)))
}}
"#
	);

	let lib_path = format!("{dir_path}/lib.rs");
	let mut file = fs::File::create(&lib_path).context(format!("Failed to create lib.rs in {dir_path}"))?;
	file.write_all(lib_content.as_bytes()).context("Failed to write to lib.rs")?;
	Ok(())
}

fn create_js_entry_point(base_dir: &str, filename: &str, component_type: &str) -> Result<()> {
	let config = read_config()?;
	let popup_entry_string = format!(
		r#"
(async () => {{
  try {{
    const src = chrome.runtime.getURL("{}.js");
    const wasmPath = chrome.runtime.getURL("{}_bg.wasm");

    const contentMain = await import(src);

    if (!contentMain.default) throw new Error("WASM entry point not found!");

    await contentMain.default({{ module_or_path: wasmPath }});

  }} catch (err) {{
    console.error("Failed to initialize WASM module:", err);
  }}
}})();
"#,
		&config.popup_name.replace("-", "_"),
		&config.popup_name.replace("-", "_")
	);

	let js_content = match component_type {
		"background" => {
			r#"// Background script entry point
import init from "/background.js";

init({ module_or_path: "/background_bg.wasm" });
"#
		},
		"content" => {
			r#"// Content script entry point
(async () => {
  try {
    const src = chrome.runtime.getURL("content.js");
    const wasmPath = chrome.runtime.getURL("content_bg.wasm");

    const contentMain = await import(src);

    if (!contentMain.default) throw new Error("WASM entry point not found!");
    await contentMain.default({ module_or_path: wasmPath });

    // attaching extract function to window
    window.contentMain = contentMain;
  } catch (err) {
    console.error("Failed to initialize WASM module:", err);
  }
})();
"#
		},
		"popup" => &popup_entry_string,
		_ => "",
	};

	let js_path = format!("{base_dir}/{filename}");
	let mut file = fs::File::create(&js_path).context(format!("Failed to create {filename} in {base_dir}"))?;
	file.write_all(js_content.as_bytes()).context(format!("Failed to write to {filename}"))?;
	Ok(())
}

fn create_html_file(base_dir: &str) -> Result<()> {
	let html_content = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Browser Extension</title>
<style>
  body {
    width: 300px;
    height: 400px;
    margin: 0;
    padding: 16px;
    font-family: sans-serif;
  }
</style>
</head>
<body>
  <div id="main"></div>
  <script type="module" src="index.js"></script>
</body>
</html>
"#;

	let html_path = format!("{base_dir}/index.html");
	let mut file = fs::File::create(&html_path).context("Failed to create index.html")?;
	file.write_all(html_content.as_bytes()).context("Failed to write to index.html")?;
	Ok(())
}

fn create_manifest_json(base_dir: &str) -> Result<()> {
	let extension_name = read_config()?.extension_directory_name;
	let manifest_content = format!(
		r#"{{
"name": "{extension_name}",
"version": "1.0",
"description": "dioxus browser extension builder extension template",
"permissions": ["activeTab", "storage", "scripting", "tabs"],
"host_permissions": ["<all_urls>"],
"content_security_policy": {{
"extension_pages": "script-src 'wasm-unsafe-eval' 'self'; object-src 'self';"
}},
"content_scripts": [
{{
  "run_at": "document_start",
  "matches": ["*://*/*"],
  "js": ["content_index.js"],
  "resources": ["content.js"]
}},
{{
  "run_at": "document_start",
  "matches": ["*://*/*"],
  "js": ["index.js"],
  "resources": ["index.js"]
}}
],
"web_accessible_resources": [
{{
  "resources": ["*.js", "*.wasm", "*.css", "snippets/**/*", "assets/**/*"],
  "matches": ["*://*/*"]
}}
],
"background": {{
"service_worker": "background_index.js",
"type": "module"
}},
"action": {{
"default_popup": "index.html",
"default_title": "User script"
}},
"manifest_version": 3
}}"#
	);

	let manifest_path = format!("{base_dir}/manifest.json");
	let mut file = fs::File::create(&manifest_path).context("Failed to create manifest.json")?;
	file.write_all(manifest_content.as_bytes()).context("Failed to write to manifest.json")?;
	Ok(())
}

pub fn setup_project_from_config() -> Result<()> {
	let config = crate::read_config()?;
	generate_project_structure(&config)?;
	Ok(())
}

// Clean the distribution directory
pub(crate) async fn clean_dist_directory(config: &ExtConfig) -> Result<()> {
	let dist_path = format!("./{}/dist", config.extension_directory_name);
	let dist_path = Path::new(&dist_path);

	if dist_path.exists() {
		info!("Cleaning dist directory: {:?}", dist_path);
		fs::remove_dir_all(dist_path).with_context(|| format!("Failed to remove dist directory: {dist_path:?}"))?;
	}

	fs::create_dir_all(dist_path).with_context(|| format!("Failed to create dist directory: {dist_path:?}"))?;

	Ok(())
}

// show build status after build
pub(crate) async fn show_final_build_report(app: Arc<Mutex<App>>) {
	let app_guard = app.lock().await;
	let (total, _, _, _) = app_guard.get_task_stats();
	let failed = app_guard.tasks.values().filter(|&&s| s == BuildStatus::Failed).count();
	let successful = app_guard.tasks.values().filter(|&&s| s == BuildStatus::Success).count();

	println!("\n--- Build Summary ---");

	match app_guard.task_state {
		BuilState::Complete { duration } => {
			let time_str =
				if duration.as_secs() >= 60 { format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60) } else { format!("{:.1}s", duration.as_secs_f32()) };
			println!("✅ Build completed successfully in {time_str}");
			println!("   Total tasks: {total}, All successful");
		},
		BuilState::Failed { duration } => {
			let time_str =
				if duration.as_secs() >= 60 { format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60) } else { format!("{:.1}s", duration.as_secs_f32()) };
			println!("❌ Build failed in {time_str}");
			println!("   Total tasks: {total}, Successful: {successful}, Failed: {failed}");

			println!("\nFailed tasks:");
			for (task_name, status) in &app_guard.tasks {
				if *status == BuildStatus::Failed {
					println!("   ❌ {task_name}");
				}
			}
		},
		_ => {
			println!("Build process was interrupted");
		},
	}
	println!("-------------------\n");
}
