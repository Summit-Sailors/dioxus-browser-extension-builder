use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::fs;
use tracing_subscriber::FmtSubscriber;
use {
	anyhow::{Context, Result},
	futures::future::{join_all, try_join_all},
	lowdash::find_uniques,
	notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher},
	std::{
		collections::HashSet,
		path::{Path, PathBuf},
		process::Stdio,
		sync::LazyLock,
		time::Duration,
	},
	strum::IntoEnumIterator,
	tokio::{
		process::Command,
		sync::{Mutex, mpsc},
	},
	tokio_util::sync::CancellationToken,
	tracing::{Level, error, info, warn},
	tracing_subscriber::fmt::{format::Writer, time::FormatTime},
};

static PENDING_BUILDS: LazyLock<Mutex<HashSet<ExtensionCrate>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static PENDING_COPIES: LazyLock<Mutex<HashSet<EFile>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

// config struct that matches the TOML structure
#[derive(Debug, Deserialize)]
struct TomlConfig {
	#[serde(rename = "extension-config")]
	extension_config: ExtConfigToml,
}

#[derive(Debug, Deserialize)]
struct ExtConfigToml {
	#[serde(rename = "assets-directory")]
	assets_directory: String,

	#[serde(rename = "background-script-index-name")]
	background_script_index_name: String,

	#[serde(rename = "content-script-index-name")]
	content_script_index_name: String,

	#[serde(rename = "extension-directory-name")]
	extension_directory_name: String,

	#[serde(rename = "popup-name")]
	popup_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExtConfig {
	background_script_index_name: String,
	content_script_index_name: String,
	extension_directory_name: String,
	popup_name: String,
	assets_dir: String,
}

fn read_config() -> Result<ExtConfig> {
	let toml_content = fs::read_to_string("dx-ext.toml").context("Failed to read dx-ext.toml file")?;

	let parsed_toml: TomlConfig = toml::from_str(&toml_content).context("Failed to parse dx-ext.toml file")?;

	// converting to our internal config structure
	Ok(ExtConfig {
		background_script_index_name: parsed_toml.extension_config.background_script_index_name,
		content_script_index_name: parsed_toml.extension_config.content_script_index_name,
		extension_directory_name: parsed_toml.extension_config.extension_directory_name,
		popup_name: parsed_toml.extension_config.popup_name,
		assets_dir: parsed_toml.extension_config.assets_directory,
	})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
enum ExtensionCrate {
	Popup,
	Background,
	Content,
}

impl ExtensionCrate {
	// the actual crate name based on config
	fn get_crate_name(&self, config: &ExtConfig) -> String {
		match self {
			Self::Popup => config.popup_name.clone(),
			Self::Background => "background".to_owned(),
			Self::Content => "content".to_owned(),
		}
	}

	async fn build_crate(self, config: &ExtConfig) -> Result<()> {
		let crate_name = self.get_crate_name(config);
		info!("Building {}...", crate_name);
		let status = Command::new("wasm-pack")
			.args([
				"build",
				"--no-pack",
				"--no-typescript",
				"--target",
				"web",
				"--out-dir",
				"../dist",
				format!("{}/{}", config.extension_directory_name, crate_name).as_ref(),
			])
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.await
			.context("Failed to execute wasm-pack")?;
		if !status.success() {
			warn!("[FAIL] wasm-pack build for {} failed with status: {}", crate_name, status);
		} else {
			info!("[SUCCESS] wasm-pack build for {}", crate_name);
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display)]
enum EFile {
	// fixed files for Chrome extensions
	Manifest,
	IndexHtml,
	IndexJs,
	// dynamic files from config
	BackgroundScript,
	ContentScript,
	Assets,
}

impl EFile {
	fn get_copy_src(&self, config: &ExtConfig) -> PathBuf {
		let base_path_binding = format!("./{}", config.extension_directory_name);
		let base_path = Path::new(&base_path_binding);
		match self {
			Self::Manifest => base_path.join("manifest.json"),
			Self::IndexHtml => base_path.join("index.html"),
			Self::IndexJs => base_path.join("index.js"),
			Self::BackgroundScript => base_path.join(&config.background_script_index_name),
			Self::ContentScript => base_path.join(&config.content_script_index_name),
			Self::Assets => base_path.join(&config.assets_dir),
		}
	}

	fn get_copy_dest(&self, config: &ExtConfig) -> PathBuf {
		let dist_path_binding = format!("./{}/dist", config.extension_directory_name);
		let dist_path = Path::new(&dist_path_binding);
		match self {
			Self::Manifest => dist_path.join("manifest.json"),
			Self::IndexHtml => dist_path.join("index.html"),
			Self::IndexJs => dist_path.join("index.js"),
			Self::BackgroundScript => dist_path.join(&config.background_script_index_name),
			Self::ContentScript => dist_path.join(&config.content_script_index_name),
			Self::Assets => dist_path.join("assets"),
		}
	}

	async fn copy_file_to_dist(self, config: &ExtConfig) -> Result<()> {
		info!("Copying {:?}...", self);
		let src = self.get_copy_src(config);
		let dest = self.get_copy_dest(config);
		let status = Command::new("cp")
			.args([
				"-fr",
				src.to_str().with_context(|| format!("Invalid source path: {src:?}"))?,
				dest.to_str().with_context(|| format!("Invalid destination path: {dest:?}"))?,
			])
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.await
			.context("Failed to execute cp")?;
		if !status.success() {
			warn!("copy for {:?} failed with status: {}", self, status);
		} else {
			info!("[SUCCESS] copy for {:?}", self);
		}
		Ok(())
	}

	// the file path string for file watching
	fn get_watch_path(&self, config: &ExtConfig) -> String {
		match self {
			Self::Manifest => "manifest.json".to_owned(),
			Self::IndexHtml => "index.html".to_owned(),
			Self::IndexJs => "index.js".to_owned(),
			Self::BackgroundScript => config.background_script_index_name.clone(),
			Self::ContentScript => config.content_script_index_name.clone(),
			Self::Assets => config.assets_dir.clone(),
		}
	}
}

struct CustomTime;

impl FormatTime for CustomTime {
	fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
		write!(w, "{}", chrono::Local::now().format("%m-%d %H:%M"))
	}
}

async fn hot_reload(config: ExtConfig) -> Result<()> {
	tokio::join!(
		join_all(ExtensionCrate::iter().map(|crate_type| async move { PENDING_BUILDS.lock().await.insert(crate_type) })),
		join_all(EFile::iter().map(|e_file| async move { PENDING_COPIES.lock().await.insert(e_file) }))
	);
	let (tx, rx) = mpsc::channel(100);
	let cancel_token = CancellationToken::new();
	let mut watcher = RecommendedWatcher::new(
		move |result: NotifyResult<Event>| {
			if let Ok(event) = result {
				if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)) {
					let _ = tx.blocking_send(event);
				}
			}
		},
		notify::Config::default(),
	)
	.context("Failed to create file watcher")?;

	let ext_dir_binding = format!("./{}", config.extension_directory_name);
	let ext_dir = Path::new(&ext_dir_binding);
	for e_file in EFile::iter() {
		watcher.watch(&ext_dir.join(e_file.get_watch_path(&config)), RecursiveMode::NonRecursive).with_context(|| format!("Failed to watch file: {e_file:?}"))?;
	}
	for e_crate in ExtensionCrate::iter() {
		watcher
			.watch(&ext_dir.join(e_crate.get_crate_name(&config)).join("src"), RecursiveMode::Recursive)
			.with_context(|| format!("Failed to watch directory: {e_crate:?}"))?;
	}
	info!("File watcher started. Press Ctrl+C to stop.");
	let watch_task = tokio::spawn(watch_loop(rx, cancel_token.clone(), config));
	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			info!("Received Ctrl+C, shutting down...");
			cancel_token.cancel();
		}
		_ = watch_task => {
			error!("Watch task unexpectedly finished");
		}
	}
	Ok(())
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// start the file watcher and build system
	Watch,
	/// build all crates and copy files without watching
	Build,
	/// create a default cofiguration file
	Init,
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();

	FmtSubscriber::builder().with_max_level(Level::INFO).with_timer(CustomTime).init();

	// configuration from TOML file
	let config = read_config().context("Failed to read configuration")?;
	info!("Using extension directory: {}", config.extension_directory_name);
	info!("Popup crate: {}", config.popup_name);
	info!("Background script: {}", config.background_script_index_name);
	info!("Content script: {}", config.content_script_index_name);
	info!("Assets directory: {}", config.assets_dir);

	match cli.command {
		Commands::Watch => {
			hot_reload(config).await?;
		},
		Commands::Build => {
			let config_clone = config.clone();
			let build_futures = ExtensionCrate::iter().map(|crate_type| crate_type.build_crate(&config));
			let copy_futures = EFile::iter().map(|e_file| e_file.copy_file_to_dist(&config_clone));

			let (build_result, copy_result) = tokio::join!(try_join_all(build_futures), try_join_all(copy_futures));
			if let Err(e) = build_result {
				error!("Error during builds: {}", e);
			}
			if let Err(e) = copy_result {
				error!("Error during copy: {}", e);
			}
		},
		Commands::Init => {
			create_default_config_toml()?;
			info!("Created default dx-ext.toml configuration file");
		},
	}

	Ok(())
}

async fn watch_loop(mut rx: mpsc::Receiver<Event>, cancel_token: CancellationToken, config: ExtConfig) {
	let mut pending_events = tokio::time::interval(Duration::from_secs(1));
	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => break,
			Some(event) = rx.recv() => {
								handle_event(&event, &config).await;
								pending_events.reset();
			}
			_ = pending_events.tick() => {
				process_pending_events(&config).await;
			}
		}
	}
}

async fn handle_event(event: &Event, config: &ExtConfig) {
	let _ext_dir = config.extension_directory_name.clone();

	let copy_futures = find_uniques(
		&event
			.paths
			.iter()
			.flat_map(|path| {
				let path_str = path.to_str().unwrap_or_default();
				EFile::iter().filter(|e_file| path_str.contains(&e_file.get_watch_path(config))).collect::<Vec<EFile>>()
			})
			.collect::<Vec<EFile>>(),
	)
	.into_iter()
	.map(|e_file| async move { PENDING_COPIES.lock().await.insert(e_file) });

	if event.paths.iter().any(|path| path.to_str().unwrap_or_default().contains("api")) {
		tokio::join!(join_all(ExtensionCrate::iter().map(|crate_type| async move { PENDING_BUILDS.lock().await.insert(crate_type) })), join_all(copy_futures));
	} else {
		tokio::join!(
			join_all(
				find_uniques(
					&event
						.paths
						.iter()
						.flat_map(|path| {
							let path_str = path.to_str().unwrap_or_default();
							ExtensionCrate::iter().filter(|e_crate| path_str.contains(&e_crate.get_crate_name(config))).collect::<Vec<ExtensionCrate>>()
						})
						.collect::<Vec<ExtensionCrate>>(),
				)
				.into_iter()
				.map(|e_crate| async move { PENDING_BUILDS.lock().await.insert(e_crate) }),
			),
			join_all(copy_futures)
		);
	}
}

async fn process_pending_events(config: &ExtConfig) {
	let builds = PENDING_BUILDS.lock().await.drain().collect::<Vec<_>>();
	let copies = PENDING_COPIES.lock().await.drain().collect::<Vec<_>>();

	let (build_result, copy_result) = tokio::join!(
		try_join_all(builds.into_iter().map(|crate_type| crate_type.build_crate(config))),
		try_join_all(copies.into_iter().map(|e_file| e_file.copy_file_to_dist(config)))
	);
	if let Err(e) = build_result {
		error!("Error during builds: {}", e);
	}
	if let Err(e) = copy_result {
		error!("Error during copy: {}", e);
	}
}

// to create a default config TOML
pub fn create_default_config_toml() -> Result<()> {
	let default_config = r#"[extension-config]
assets-directory = "popup/assets"                    # your assets directory relative to the extension directory
background-script-index-name = "background_index.js" # name of your background script entry point
content-script-index-name = "content_index.js"       # name of your content script entry point
extension-directory-name = "demo-extension"          # name of your extension directory
popup-name = "popup"                                 # name of your popup crate
"#;

	fs::write("dx-ext.toml", default_config).context("Failed to write default dx-ext.toml file")
}
