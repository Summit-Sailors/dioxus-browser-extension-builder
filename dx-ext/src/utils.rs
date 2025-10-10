use stilts::Template;
use {
	crate::{
		App,
		common::{BuildMode, BuildState, ExtConfig, InitOptions, TaskStatus, TomlConfig},
	},
	anyhow::{Context, Result},
	dialoguer::{Confirm, Input},
	std::{fs, io::Write, path::Path, sync::Arc},
	tokio::sync::Mutex,
	tracing::info,
};

#[derive(Template)]
#[stilts(path = "workspace_cargo.toml.j2")]
struct WorkspaceCargoToml<'s> {
	directory_name: &'s str,
	popup_name: &'s str,
}

#[derive(Template)]
#[stilts(path = "crate_cargo.toml.j2")]
struct CrateCargoToml<'s> {
	crate_name: &'s str,
}

#[derive(Template)]
#[stilts(path = "gitignore.j2")]
struct GitIgnore {}

#[derive(Template)]
#[stilts(path = "lib_rs.rs.j2")]
struct LibRs<'s> {
	component_name: &'s str,
}

#[derive(Template)]
#[stilts(path = "popup_entry.js.j2")]
struct PopupEntry<'s> {
	popup_name: &'s str,
}

#[derive(Template)]
#[stilts(path = "background_entry.js.j2")]
struct BackgroundEntry {}

#[derive(Template)]
#[stilts(path = "content_entry.js.j2")]
struct ContentEntry {}

#[derive(Template)]
#[stilts(path = "index.html.j2")]
struct IndexHtml {}

#[derive(Template)]
#[stilts(path = "manifest.json.j2")]
struct ManifestJson {
	extension_name: String,
}

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
	let assets_dir = get_interactive_or_default("Enter assets directory", format!("{popup_name}/assets").as_str())?;
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

fn create_workspace_cargo_toml() -> Result<()> {
	let config = read_config()?;
	let cargo_content = WorkspaceCargoToml { directory_name: &config.extension_directory_name, popup_name: &config.popup_name }.render()?;
	let pwd = std::env::current_dir()?;
	let cargo_path = pwd.join("Cargo.toml");
	let mut file = fs::File::create(&cargo_path).context("Failed to create workspace Cargo.toml".to_owned())?;
	file.write_all(cargo_content.as_bytes()).context("Failed to write to Cargo.toml")?;
	Ok(())
}

fn init_git() -> Result<()> {
	let gitignore_content = GitIgnore {}.render()?;
	let pwd = std::env::current_dir()?;
	let gitignore_path = pwd.join(".gitignore");
	let mut file = fs::File::create(&gitignore_path).context("Failed to create workspace Cargo.toml".to_owned())?;
	file.write_all(gitignore_content.as_bytes()).context("Failed to write to Cargo.toml")?;
	let _ = std::process::Command::new("git").arg("init").output()?;
	Ok(())
}

fn create_cargo_toml(dir_path: &str, crate_name: &str) -> Result<()> {
	let cargo_content = CrateCargoToml { crate_name }.render()?;

	let cargo_path = format!("{dir_path}/Cargo.toml");
	let mut file = fs::File::create(&cargo_path).context(format!("Failed to create Cargo.toml in {dir_path}"))?;
	file.write_all(cargo_content.as_bytes()).context("Failed to write to Cargo.toml")?;
	Ok(())
}

fn create_lib_rs(dir_path: &str, component_name: &str) -> Result<()> {
	let lib_content = LibRs { component_name }.render()?;
	let lib_path = format!("{dir_path}/lib.rs");
	let mut file = fs::File::create(&lib_path).context(format!("Failed to create lib.rs in {dir_path}"))?;
	file.write_all(lib_content.as_bytes()).context("Failed to write to lib.rs")?;
	Ok(())
}

fn create_js_entry_point(base_dir: &str, filename: &str, component_type: &str) -> Result<()> {
	let config = read_config()?;
	let js_content = match component_type {
		"background" => BackgroundEntry {}.render()?,
		"content" => ContentEntry {}.render()?,
		"popup" => PopupEntry { popup_name: &config.popup_name.replace("-", "_") }.render()?,
		_ => String::new(),
	};
	let js_path = format!("{base_dir}/{filename}");
	let mut file = fs::File::create(&js_path).context(format!("Failed to create {filename} in {base_dir}"))?;
	file.write_all(js_content.as_bytes()).context(format!("Failed to write to {filename}"))?;
	Ok(())
}

fn create_html_file(base_dir: &str) -> Result<()> {
	let html_content = IndexHtml {}.render()?;
	let html_path = format!("{base_dir}/index.html");
	let mut file = fs::File::create(&html_path).context("Failed to create index.html")?;
	file.write_all(html_content.as_bytes()).context("Failed to write to index.html")?;
	Ok(())
}

fn create_manifest_json(base_dir: &str) -> Result<()> {
	let manifest_content = ManifestJson { extension_name: read_config()?.extension_directory_name }.render()?;
	let manifest_path = format!("{base_dir}/manifest.json");
	let mut file = fs::File::create(&manifest_path).context("Failed to create manifest.json")?;
	file.write_all(manifest_content.as_bytes()).context("Failed to write to manifest.json")?;
	Ok(())
}

pub fn setup_project_from_config() -> Result<()> {
	let config = crate::read_config()?;
	generate_project_structure(&config)?;
	create_workspace_cargo_toml()?;
	init_git()?;
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
	let stats = app_guard.get_task_stats();
	let failed = app_guard.tasks.values().filter(|&&s| s == TaskStatus::Failed).count();
	let successful = app_guard.tasks.values().filter(|&&s| s == TaskStatus::Success).count();
	println!("\n--- Build Summary ---");
	match app_guard.task_state {
		BuildState::Complete { duration } => {
			let time_str =
				if duration.as_secs() >= 60 { format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60) } else { format!("{:.1}s", duration.as_secs_f32()) };
			let all_tasks = stats.total;
			println!("✅ Build completed successfully in {time_str}");
			println!("   Total tasks: {all_tasks}, All successful");
		},
		BuildState::Failed { duration } => {
			let time_str =
				if duration.as_secs() >= 60 { format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60) } else { format!("{:.1}s", duration.as_secs_f32()) };
			let all_tasks = stats.total;
			println!("❌ Build failed in {time_str}");
			println!("   Total tasks: {all_tasks}, Successful: {successful}, Failed: {failed}");
			println!("\nFailed tasks:");
			for (task_name, status) in &app_guard.tasks {
				if *status == TaskStatus::Failed {
					println!("   ❌ {task_name}");
				}
			}
		},
		_ => println!("Build process was interrupted"),
	}
	println!("-------------------\n");
}
