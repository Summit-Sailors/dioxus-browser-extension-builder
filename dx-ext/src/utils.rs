use {
	crate::common::{BuildMode, ExtConfig, InitOptions, TomlConfig},
	anyhow::{Context, Result},
	dialoguer::{Confirm, Input},
	std::{fs, path::Path},
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

pub(crate) fn create_default_config_toml(options: &InitOptions) -> Result<()> {
	info!("Welcome to the Dioxus Browser Extension Builder Setup");

	if Path::new("dx-ext.toml").exists() && !options.force {
		info!("Config file already exists. Use --force to overwrite.");
		return Ok(());
	}

	let extension_dir = if options.interactive {
		Input::new().with_prompt("Enter extension directory name").default(options.extension_dir.clone()).interact_text()?
	} else {
		options.extension_dir.clone()
	};

	let popup_name = if options.interactive {
		Input::new().with_prompt("Enter popup crate name").default(options.popup_name.clone()).interact_text()?
	} else {
		options.popup_name.clone()
	};

	let background_script = if options.interactive {
		Input::new().with_prompt("Enter background script entry point").default(options.background_script.clone()).interact_text()?
	} else {
		options.background_script.clone()
	};

	let content_script = if options.interactive {
		Input::new().with_prompt("Enter content script entry point").default(options.content_script.clone()).interact_text()?
	} else {
		options.content_script.clone()
	};

	let enable_incremental_builds = if options.interactive {
		Confirm::new().with_prompt("Enable incremental builds?").default(options.enable_incremental_builds).interact()?
	} else {
		options.enable_incremental_builds
	};

	let assets_dir = if options.interactive {
		Input::new().with_prompt("Enter assets directory").default(options.assets_dir.clone()).interact_text()?
	} else {
		options.assets_dir.clone()
	};

	let config_content = format!(
		r#"[extension-config]
assets-directory = "{assets_dir}"
background-script-index-name = "{background_script}"
content-script-index-name = "{content_script}"
extension-directory-name = "{extension_dir}"
popup-name = "{popup_name}"
enable-incremental-builds = {}
"#,
		enable_incremental_builds
	);

	fs::write("dx-ext.toml", config_content).context("Failed to write dx-ext.toml file")?;

	info!("Configuration created successfully:");
	info!("  Extension directory: {extension_dir}");
	info!("  Popup crate: {popup_name}");
	info!("  Background script: {background_script}");
	info!("  Content script: {content_script}");
	info!("  Assets directory: {assets_dir}");
	info!("  Enable incremental builds: {}", enable_incremental_builds);

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
