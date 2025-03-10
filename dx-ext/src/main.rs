//! # dx-ext
//!
//! A CLI tool for building browser extension using Dioxus
//! `dx-ext` simplifies the development workflow for creating browser extensions with Dioxus
//!
//! ## Commands
//!
//! ### Init
//!
//! Creates a bew configuration file (`dx-ext.toml`) with customizable options.
//!
//! ```bash
//! dx-ext init [OPTIONS]
//! ```
//!
//! Options:
//! - `--extension-dir <DIR>`: Name of the extension directory (default: "extension")
//! - `--popup name <NAME>`: Name of the popup crate (default: "popup")
//! - `--background-script <FILE>`: Name of the background script entry point (default: "`background_index.js`")
//! - `--content-script <FILE>`: Name of the content script entry point (default: "`content_index.js`")
//! - `--assets-dir <DIR>`: Assets directory path relative to the extension's directory (default: "popup/assets")
//! - `-f, --force`: Force overwrite of the existing config file
//! - `-i, --interactive`: Interactive mode to collect confiuration information
//! - `--mode, -m`: Build mode: development or release (default: "development")
//! - `--clean, -c`: Clean build (remove dist directory first)
//!
//! ### Build
//!
//! Builds all crates and copies all necessary files to the `dist` directory
//!
//! ```bash
//! dx-ext build
//!
//! dx-ext build -m release # Release mode builds
//!
//! dx-ext build --clean # clean builds
//! ```
//!
//! ### Watch
//!
//! Starts a file watcher and builds the extension automatically when files change.
//!
//! ```bash
//! dx-ext watch
//! ```
//!
//! ## Configuration:
//!
//! The tool uses a `dx-ext.toml` file in the project root with the following structure:
//!
//! ```toml
//! [extension-config]
//! assets-directory = "popup/assets"                   # your assets directory relative to the extension directory
//! background-script-index-name = "background_index.js"       # name of your background script entry point
//! content-script-index-name = "content_index.js"          # name of your content script entry point
//! extension-directory-name = "extension"            # name of your extension directory
//! popup-name = "popup"                          # name of your popup crate
//! ```
//!
//! ## Internal Structure
//!
//! The tool organizes extension components into three main crates:
//! - `Popup`: The UI component of the extension
//! - `Background`: The background script that runs persistently
//! - `Content`: The content script that runs in the context of web pages
//!
//! File operations are managed through the `EFile` enum which handles copying:
//! - `Manifest`: The extension's manifest.json
//! - `IndexHtml`: Main HTML file
//! - `IndexJs`: Main JavaScript entry point
//! - `BackgroundScript`: The background script entry point
//! - `ContentScript`: The content script entry point
//! - `Assets`: Additional assets required by the extension

use {
	anyhow::{Context, Result},
	clap::{ArgAction, Args, Parser, Subcommand, ValueHint, command},
	dialoguer::Input,
	futures::future::{join_all, try_join_all},
	lowdash::find_uniques,
	notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher},
	serde::{Deserialize, Serialize},
	std::{
		collections::{HashMap, HashSet},
		fs, io,
		path::{Path, PathBuf},
		process::Stdio,
		sync::LazyLock,
		time::{Duration, SystemTime},
	},
	strum::IntoEnumIterator,
	tokio::{
		process::Command,
		sync::{Mutex, mpsc},
	},
	tokio_util::sync::CancellationToken,
	tracing::{Level, debug, error, info, warn},
	tracing_subscriber::{
		FmtSubscriber,
		fmt::{format::Writer, time::FormatTime},
	},
};

static PENDING_BUILDS: LazyLock<Mutex<HashSet<ExtensionCrate>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static PENDING_COPIES: LazyLock<Mutex<HashSet<EFile>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static FILE_HASHES: LazyLock<Mutex<HashMap<PathBuf, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static FILE_TIMESTAMPS: LazyLock<Mutex<HashMap<PathBuf, SystemTime>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum BuildMode {
	Development,
	Release,
}

impl std::fmt::Display for BuildMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Development => write!(f, "development"),
			Self::Release => write!(f, "release"),
		}
	}
}

impl std::str::FromStr for BuildMode {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"development" | "dev" => Ok(Self::Development),
			"release" | "prod" | "production" => Ok(Self::Release),
			_ => Err(format!("Invalid build mode: {s}. Use 'development' or 'release'")),
		}
	}
}

// config struct that matches the TOML structure
#[derive(Debug, Deserialize, Serialize)]
struct TomlConfig {
	#[serde(rename = "extension-config")]
	extension_config: ExtConfigToml,
}

#[derive(Debug, Deserialize, Serialize)]
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
	build_mode: BuildMode,
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
		build_mode: BuildMode::Development,
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
		info!("Building {} in {} mode...", crate_name, config.build_mode);

		let mut args = vec![
			"build".to_owned(),
			"--no-pack".to_owned(),
			"--no-typescript".to_owned(),
			"--target".to_owned(),
			"web".to_owned(),
			"--out-dir".to_owned(),
			"../dist".to_owned(),
		];

		// profile flag based on build mode
		if matches!(config.build_mode, BuildMode::Release) {
			args.push("--release".to_owned());
		}

		args.push(format!("{}/{}", config.extension_directory_name, crate_name));

		let status = Command::new("wasm-pack").args(&args).stdout(Stdio::null()).stderr(Stdio::null()).status().await.context("Failed to execute wasm-pack")?;

		if !status.success() {
			warn!("[FAIL] wasm-pack build for {} failed with status: {}", crate_name, status);
			return Err(anyhow::anyhow!("Build failed for {}", crate_name));
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

	fn calculate_file_hash(path: &Path) -> Result<String> {
		let mut file = fs::File::open(path).with_context(|| format!("Failed to open file for hashing: {path:?}"))?;
		let mut hasher = blake3::Hasher::new();

		let mut buffer = [0; 8192];
		loop {
			let bytes_read = io::Read::read(&mut file, &mut buffer).with_context(|| format!("Failed to read file for hashing: {path:?}"))?;

			if bytes_read == 0 {
				break;
			}

			hasher.update(&buffer[..bytes_read]);
		}

		Ok(hasher.finalize().to_hex().to_string())
	}

	// hash checking to avoid unnecessary copies
	async fn copy_file(src: &Path, dest: &Path) -> Result<bool> {
		if !src.exists() {
			return Err(anyhow::anyhow!("Source file does not exist: {:?}", src));
		}

		let mut hashes = FILE_HASHES.lock().await;
		let mut timestamps = FILE_TIMESTAMPS.lock().await;

		// check if destination exists and get its metadata
		let copy_needed = if dest.exists() {
			let src_metadata = fs::metadata(src).with_context(|| format!("Failed to get metadata for source file: {src:?}"))?;
			let dest_metadata = fs::metadata(dest).with_context(|| format!("Failed to get metadata for destination file: {dest:?}"))?;

			// check if sizes differ - quick check before hashing
			if src_metadata.len() != dest_metadata.len() {
				true
			} else if let Ok(src_time) = src_metadata.modified() {
				// if we have a stored timestamp for the source file
				if let Some(stored_time) = timestamps.get(src) {
					if *stored_time == src_time {
						// file hasn't changed since last check
						false
					} else {
						// file changed, check content hash
						let src_hash = Self::calculate_file_hash(src)?;
						let dest_hash = Self::calculate_file_hash(dest)?;

						if src_hash != dest_hash {
							// update the hash
							hashes.insert(src.to_path_buf(), src_hash);
							timestamps.insert(src.to_path_buf(), src_time);
							true
						} else {
							// update timestamp even though content hasn't changed
							timestamps.insert(src.to_path_buf(), src_time);
							false
						}
					}
				} else {
					// no stored timestamp, compare hashes
					let src_hash = Self::calculate_file_hash(src)?;
					let dest_hash = Self::calculate_file_hash(dest)?;

					if src_hash != dest_hash {
						hashes.insert(src.to_path_buf(), src_hash);
						timestamps.insert(src.to_path_buf(), src_time);
						true
					} else {
						timestamps.insert(src.to_path_buf(), src_time);
						false
					}
				}
			} else {
				// if we can't get modification time, fall back to hash comparison
				let src_hash = Self::calculate_file_hash(src)?;
				let dest_hash = Self::calculate_file_hash(dest)?;

				hashes.insert(src.to_path_buf(), src_hash.clone());
				src_hash != dest_hash
			}
		} else {
			// destination doesn't exist, copy is needed
			// also store the hash for future comparisons
			if let Ok(src_metadata) = fs::metadata(src) {
				if let Ok(modified) = src_metadata.modified() {
					timestamps.insert(src.to_path_buf(), modified);
				}
			}

			if let Ok(hash) = Self::calculate_file_hash(src) {
				hashes.insert(src.to_path_buf(), hash);
			}

			true
		};

		if copy_needed {
			// create parent directories if they don't exist
			if let Some(parent) = dest.parent() {
				fs::create_dir_all(parent).with_context(|| format!("Failed to create parent directory: {parent:?}"))?;
			}

			// the actual copy
			fs::copy(src, dest).with_context(|| format!("Failed to copy file from {src:?} to {dest:?}"))?;

			debug!("Copied file: {:?} -> {:?}", src, dest);
			Ok(true)
		} else {
			debug!("Skipped copying unchanged file: {:?}", src);
			Ok(false)
		}
	}

	// directory copy with parallel processing and hash checking
	async fn copy_dir_all(src: &Path, dst: &Path) -> Result<bool> {
		if !src.exists() {
			return Err(anyhow::anyhow!("Source directory does not exist: {:?}", src));
		}

		// create destination directory if it doesn't exist
		fs::create_dir_all(dst).with_context(|| format!("Failed to create destination directory: {dst:?}"))?;

		// directory entries read
		let entries = fs::read_dir(src).with_context(|| format!("Failed to read source directory: {src:?}"))?.collect::<Result<Vec<_>, _>>()?;

		let copy_futures = entries.into_iter().map(|entry| {
			let src_path = entry.path();
			let dst_path = dst.join(entry.file_name());

			async move {
				let ty = entry.file_type().with_context(|| format!("Failed to get file type for: {src_path:?}"))?;

				if ty.is_dir() {
					Self::copy_dir_all(&src_path, &dst_path).await
				} else if ty.is_file() {
					Self::copy_file(&src_path, &dst_path).await
				} else {
					// skip symlinks and other special files
					debug!("Skipping non-regular file: {:?}", src_path);
					Ok(false)
				}
			}
		});

		let results = try_join_all(copy_futures).await?;

		// true if any files were copied
		Ok(results.into_iter().any(|copied| copied))
	}

	async fn copy_file_to_dist(self, config: &ExtConfig) -> Result<()> {
		info!("Copying {:?}...", self);
		let src = self.get_copy_src(config);
		let dest = self.get_copy_dest(config);

		let result = if src.is_dir() { Self::copy_dir_all(&src, &dest).await } else { Self::copy_file(&src, &dest).await };

		match result {
			Ok(copied) => {
				if copied {
					info!("[SUCCESS] Copied {:?}", self);
				} else {
					info!("[SKIPPED] No changes for {:?}", self);
				}
				Ok(())
			},
			Err(e) => {
				warn!("Copy for {:?} failed: {}", self, e);
				Err(e)
			},
		}
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
		let watch_path = ext_dir.join(e_file.get_watch_path(&config));
		if watch_path.exists() {
			watcher.watch(&watch_path, RecursiveMode::NonRecursive).with_context(|| format!("Failed to watch file: {e_file:?} at path {watch_path:?}"))?;
		} else {
			warn!("Watch path does not exist: {:?}", watch_path);
		}
	}
	for e_crate in ExtensionCrate::iter() {
		let crate_src_path = ext_dir.join(e_crate.get_crate_name(&config)).join("src");
		if crate_src_path.exists() {
			watcher.watch(&crate_src_path, RecursiveMode::Recursive).with_context(|| format!("Failed to watch directory: {e_crate:?} at path {crate_src_path:?}"))?;
		} else {
			warn!("Crate source path does not exist: {:?}", crate_src_path);
		}
	}
	info!("File watcher started in {} mode. Press Ctrl+C to stop.", config.build_mode);
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

// Configuration options for the Init command
#[derive(Args, Debug)]
struct InitOptions {
	/// Extension directory name
	#[arg(long, help = "Name of your extension directory", default_value = "extension", value_hint = ValueHint::DirPath)]
	extension_dir: String,

	/// Popup crate name
	#[arg(long, help = "Name of your popup crate", default_value = "popup")]
	popup_name: String,

	/// Background script entry point
	#[arg(long, help = "Name of your background script entry point", default_value = "background_index.js")]
	background_script: String,

	/// Content script entry point
	#[arg(long, help = "Name of your content script entry point", default_value = "content_index.js")]
	content_script: String,

	/// Assets directory
	#[arg(long, help = "Your assets directory relative to the extension directory", default_value = "popup/assets", value_hint = ValueHint::DirPath)]
	assets_dir: String,

	/// Force overwrite existing config file
	#[arg(short, long, help = "Force overwrite of existing config file", action = ArgAction::SetTrue)]
	force: bool,

	/// Interactive mode to collect configuration
	#[arg(short, long, help = "Interactive mode to collect configuration", action = ArgAction::SetTrue)]
	interactive: bool,
}

// Build options shared by Build and Watch commands
#[derive(Args, Debug, Clone)]
struct BuildOptions {
	/// Build mode (development or release)
	#[arg(short, long, help = "Build mode: development or release", default_value = "development")]
	mode: BuildMode,

	/// Clean build (remove dist directory before building)
	#[arg(short, long, help = "Clean build (remove dist directory first)", action = ArgAction::SetTrue)]
	clean: bool,
}

#[derive(Parser)]
#[command(name = "dx-ext", author = "Summit Sailors", version, about = "CLI tool for building browser extensions using dioxus", long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Start the file watcher and build system
	Watch(BuildOptions),
	/// Build all crates and copy files without watching
	Build(BuildOptions),
	/// Create a configuration file with customizable options
	Init(InitOptions),
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();

	FmtSubscriber::builder().with_max_level(Level::INFO).with_timer(CustomTime).init();

	match cli.command {
		Commands::Watch(options) => {
			// configuration from TOML file with build mode override
			let mut config = read_config().context("Failed to read configuration")?;
			config.build_mode = options.mode;

			info!("Using extension directory: {}", config.extension_directory_name);
			info!("Popup crate: {}", config.popup_name);
			info!("Background script: {}", config.background_script_index_name);
			info!("Content script: {}", config.content_script_index_name);
			info!("Assets directory: {}", config.assets_dir);
			info!("Build mode: {}", config.build_mode);

			if options.clean {
				clean_dist_directory(&config).await?;
			}

			hot_reload(config).await?;
		},
		Commands::Build(options) => {
			// configuration from TOML file with build mode override
			let mut config = read_config().context("Failed to read configuration")?;
			config.build_mode = options.mode;

			info!("Using extension directory: {}", config.extension_directory_name);
			info!("Popup crate: {}", config.popup_name);
			info!("Background script: {}", config.background_script_index_name);
			info!("Content script: {}", config.content_script_index_name);
			info!("Assets directory: {}", config.assets_dir);
			info!("Build mode: {}", config.build_mode);

			if options.clean {
				clean_dist_directory(&config).await?;
			}

			let build_futures = ExtensionCrate::iter().map(|crate_type| crate_type.build_crate(&config));
			let copy_futures = EFile::iter().map(|e_file| e_file.copy_file_to_dist(&config));

			let (build_result, copy_result) = tokio::join!(try_join_all(build_futures), try_join_all(copy_futures));
			if let Err(e) = build_result {
				error!("Error during builds: {}", e);
			}
			if let Err(e) = copy_result {
				error!("Error during copy: {}", e);
			}
		},
		Commands::Init(options) => {
			create_default_config_toml(&options)?;
			info!("Created dx-ext.toml configuration file");
		},
	}

	Ok(())
}

// Clean the distribution directory
async fn clean_dist_directory(config: &ExtConfig) -> Result<()> {
	let dist_path = format!("./{}/dist", config.extension_directory_name);
	let dist_path = Path::new(&dist_path);

	if dist_path.exists() {
		info!("Cleaning dist directory: {:?}", dist_path);
		fs::remove_dir_all(dist_path).with_context(|| format!("Failed to remove dist directory: {dist_path:?}"))?;
	}

	fs::create_dir_all(dist_path).with_context(|| format!("Failed to create dist directory: {dist_path:?}"))?;

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
	// optimization: Skip processing for temporary files and other non-relevant files
	if event.paths.iter().any(|path| {
		let path_str = path.to_string_lossy();
		path_str.contains(".tmp") || path_str.contains(".swp") || path_str.contains("~") || path_str.ends_with(".git")
	}) {
		info!("Skipping temporary or non-relevant file: {:?}", event.paths);
		return;
	}

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

	// shared API changes
	if event.paths.iter().any(|path| path.to_str().unwrap_or_default().contains("api")) {
		tokio::join!(join_all(ExtensionCrate::iter().map(|crate_type| async move { PENDING_BUILDS.lock().await.insert(crate_type) })), join_all(copy_futures));
	} else {
		// crate-specific changes
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

	if builds.is_empty() && copies.is_empty() {
		return;
	}

	let build_label = if builds.len() == 1 {
		format!("{}", builds[0])
	} else if builds.len() <= 3 {
		builds.iter().map(|b| format!("{b}")).collect::<Vec<_>>().join(", ")
	} else {
		format!("{} crates", builds.len())
	};

	let copy_label = if copies.len() == 1 {
		format!("{}", copies[0])
	} else if copies.len() <= 3 {
		copies.iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(", ")
	} else {
		format!("{} files", copies.len())
	};

	if !builds.is_empty() {
		info!("Processing {} build(s): {}", builds.len(), build_label);
	}

	if !copies.is_empty() {
		info!("Processing {} copy operation(s): {}", copies.len(), copy_label);
	}

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

// to create a config TOML with user input from clap
fn create_default_config_toml(options: &InitOptions) -> Result<()> {
	println!("Welcome to the Dioxus Browser Extension Builder Setup");

	if Path::new("dx-ext.toml").exists() && !options.force {
		println!("Config file already exists. Use --force to overwrite.");
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

	let assets_dir = if options.interactive {
		Input::new().with_prompt("Enter assets directory").default(options.assets_dir.clone()).interact_text()?
	} else {
		options.assets_dir.clone()
	};

	let config_content = format!(
		r#"[extension-config]
assets-directory = "{assets_dir}"                    # your assets directory relative to the extension directory
background-script-index-name = "{background_script}"        # name of your background script entry point
content-script-index-name = "{content_script}"           # name of your content script entry point
extension-directory-name = "{extension_dir}"            # name of your extension directory
popup-name = "{popup_name}"                          # name of your popup crate
"#
	);

	fs::write("dx-ext.toml", config_content).context("Failed to write dx-ext.toml file")?;

	println!("Configuration created successfully:");
	println!("  Extension directory: {extension_dir}");
	println!("  Popup crate: {popup_name}");
	println!("  Background script: {background_script}");
	println!("  Content script: {content_script}");
	println!("  Assets directory: {assets_dir}");

	Ok(())
}
