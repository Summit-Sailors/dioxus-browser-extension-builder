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

mod app;
mod common;
mod efile;
mod extcrate;
mod terminal;
mod utils;

use {
	anyhow::{Context, Result},
	app::App,
	clap::{ArgAction, Args, Parser, Subcommand},
	common::{BuildMode, BuildStatus, EXMessage, ExtConfig, InitOptions, PENDING_BUILDS, PENDING_COPIES},
	crossterm::{
		ExecutableCommand,
		cursor::Show,
		event::{self, KeyCode, KeyEventKind, KeyModifiers},
		terminal::disable_raw_mode,
	},
	efile::EFile,
	extcrate::ExtensionCrate,
	futures::future::{join_all, try_join_all},
	lazy_static::lazy_static,
	lowdash::find_uniques,
	notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher},
	std::{
		io::stdout,
		path::Path,
		sync::{Arc, Mutex},
		time::Duration,
	},
	strum::IntoEnumIterator,
	terminal::Terminal,
	tokio::sync::mpsc,
	tokio_util::sync::CancellationToken,
	tracing::{Level, debug, error, info, warn},
	tracing_subscriber::{
		FmtSubscriber,
		fmt::{format::Writer, time::FormatTime},
	},
	utils::{clean_dist_directory, create_default_config_toml, read_config},
};

lazy_static! {
	pub(crate) static ref UI_SENDER: Mutex<Option<mpsc::UnboundedSender<EXMessage>>> = Mutex::new(None);
}

const TICK_RATE_MS: u64 = 100;

struct CustomTime;

impl FormatTime for CustomTime {
	fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
		write!(w, "{}", chrono::Local::now().format("%m-%d %H:%M"))
	}
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
	#[clap(name = "watch")]
	Watch(BuildOptions),
	/// Build all crates and copy files without watching
	#[clap(name = "build")]
	Build(BuildOptions),
	/// Create a configuration file with customizable options
	#[clap(name = "init")]
	Init(InitOptions),
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	FmtSubscriber::builder().with_max_level(Level::INFO).with_timer(CustomTime).init();

	// panic hook to restore terminal on panic
	let original_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		_ = disable_raw_mode();
		_ = stdout().execute(Show);
		original_hook(info);
	}));

	match cli.command {
		Commands::Watch(options) => {
			// configuration from TOML file with build mode override
			let mut config = read_config().context("Failed to read configuration")?;
			config.build_mode = options.mode;

			debug!("Using extension directory: {}", config.extension_directory_name);
			debug!("Popup crate: {}", config.popup_name);
			debug!("Background script: {}", config.background_script_index_name);
			debug!("Content script: {}", config.content_script_index_name);
			debug!("Assets directory: {}", config.assets_dir);
			debug!("Build mode: {}", config.build_mode);

			if options.clean {
				clean_dist_directory(&config).await?;
			}

			let (app, terminal, ui_rx) = setup_tui().await?;
			hot_reload(config, app, terminal, ui_rx).await?;
		},
		Commands::Build(options) => {
			// configuration from TOML file with build mode override
			let mut config = read_config().context("Failed to read configuration")?;
			config.build_mode = options.mode;

			debug!("Using extension directory: {}", config.extension_directory_name);
			debug!("Popup crate: {}", config.popup_name);
			debug!("Background script: {}", config.background_script_index_name);
			debug!("Content script: {}", config.content_script_index_name);
			debug!("Assets directory: {}", config.assets_dir);
			debug!("Build mode: {}", config.build_mode);

			if options.clean {
				clean_dist_directory(&config).await?;
			}

			let (app, terminal, ui_rx) = setup_tui().await?;

			for e_crate in ExtensionCrate::iter() {
				app.lock().unwrap().tasks.insert(e_crate.get_task_name(), BuildStatus::Pending);
				update_task_status(&e_crate.get_task_name(), BuildStatus::Pending).await;
			}

			let app_clone = app.clone();
			let cancel_token = CancellationToken::new();
			let ui_cancel_token = cancel_token.clone();

			let ui_task = tokio::spawn(async move { run_ui_loop(app_clone, terminal, ui_rx, ui_cancel_token).await });

			for e_crate in ExtensionCrate::iter() {
				update_task_status(&e_crate.get_task_name(), BuildStatus::InProgress).await;

				match e_crate.build_crate(&config).await {
					Ok(_) => update_task_status(&e_crate.get_task_name(), BuildStatus::Success).await,
					Err(e) => {
						error!("Failed to build {}: {}", e_crate.get_task_name(), e);
						update_task_status(&e_crate.get_task_name(), BuildStatus::Failed).await;
					},
				}
			}

			for e_file in EFile::iter() {
				if let Err(e) = e_file.copy_file_to_dist(&config).await {
					error!("Failed to copy file: {}", e);
				}
			}

			// cancel UI and wait for it to complete
			cancel_token.cancel();
			let _ = ui_task.await;
		},
		Commands::Init(options) => {
			create_default_config_toml(&options)?;
			info!("Created dx-ext.toml configuration file");
		},
	}

	Ok(())
}

fn initialize_sender() -> mpsc::UnboundedReceiver<EXMessage> {
	let (tx, rx) = mpsc::unbounded_channel();
	let mut sender = UI_SENDER.lock().unwrap();
	*sender = Some(tx);
	rx
}

async fn send_ui_message(message: EXMessage) {
	let sender = UI_SENDER.lock().unwrap();
	if let Some(tx) = sender.as_ref() {
		if let Err(e) = tx.send(message) {
			error!("Error sending message: {}", e);
		}
	} else {
		error!("Sender not initialized");
	}
}

async fn setup_tui() -> Result<(Arc<Mutex<App>>, Terminal, mpsc::UnboundedReceiver<EXMessage>)> {
	let app = Arc::new(Mutex::new(App::new()));
	let terminal = Terminal::new()?;
	let ui_rx = initialize_sender();

	Ok((app, terminal, ui_rx))
}

async fn update_task_status(task_name: &str, status: BuildStatus) {
	send_ui_message(EXMessage::UpdateTask(task_name.to_string(), status)).await;
}

async fn hot_reload(config: ExtConfig, app: Arc<Mutex<App>>, terminal: Terminal, ui_rx: mpsc::UnboundedReceiver<EXMessage>) -> Result<()> {
	// init with existing crates
	for e_crate in ExtensionCrate::iter() {
		app.lock().unwrap().tasks.insert(e_crate.get_task_name(), BuildStatus::Pending);
	}

	tokio::join!(
		join_all(ExtensionCrate::iter().map(|crate_type| async move {
			PENDING_BUILDS.lock().await.insert(crate_type);
			update_task_status(&crate_type.get_task_name(), BuildStatus::Pending).await;
		})),
		join_all(EFile::iter().map(|e_file| async move {
			PENDING_COPIES.lock().await.insert(e_file);
		}))
	);

	let (tx, rx) = mpsc::channel(100);
	let cancel_token = CancellationToken::new();
	let watch_cancel_token = cancel_token.clone();
	let ui_cancel_token = cancel_token.clone();

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

	// watch task
	let config_clone = config.clone();
	let watch_task = tokio::spawn(async move {
		watch_loop(rx, watch_cancel_token, config_clone).await;
	});

	// UI event loop
	let ui_task = tokio::spawn(async move { run_ui_loop(app, terminal, ui_rx, ui_cancel_token).await });

	tokio::select! {
		_ = watch_task => {
			warn!("Watch task completed unexpectedly");
		}
		result = ui_task => {
			if let Err(e) = result {
				error!("UI task error: {:?}", e);
			}
		}
	}

	cancel_token.cancel();
	Ok(())
}

async fn run_ui_loop(
	app: Arc<Mutex<App>>,
	mut terminal: Terminal,
	mut ui_rx: mpsc::UnboundedReceiver<EXMessage>,
	cancel_token: CancellationToken,
) -> Result<()> {
	let mut interval = tokio::time::interval(Duration::from_millis(TICK_RATE_MS));

	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => break,
			_ = interval.tick() => {
				// UI updates handler
				{
					let mut app = app.lock().unwrap();
					app.update(EXMessage::Tick);
				}

				// UI draw
				{
					let mut app_guard = app.lock().unwrap();
					if app_guard.should_quit {
						break;
					}
					if let Err(e) = terminal.draw(&mut app_guard) {
						error!("Failed to draw UI: {}", e);
						break;
					}
				}

				// checking for key events
				if crossterm::event::poll(Duration::from_millis(0))? {
					if let crossterm::event::Event::Key(key) = event::read()? {
						if key.kind == KeyEventKind::Press {
							{
									let mut app = app.lock().unwrap();
									app.update(EXMessage::Keypress(key.code));
							}

							if key.code == KeyCode::Char('r') { } // to be handled

							if key.code == KeyCode::Char('q') ||
								(key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)) {
									let mut app = app.lock().unwrap();
									app.should_quit = true;
							}
						}
					}
				}
			}
			Some(ui_msg) = ui_rx.recv() => {
				let mut app = app.lock().unwrap();
				app.update(ui_msg);
			}
		}
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
		let builds = ExtensionCrate::iter().collect::<Vec<_>>();

		// update UI for each build crate
		for crate_type in &builds {
			update_task_status(&crate_type.get_task_name(), BuildStatus::Pending).await;
		}

		tokio::join!(join_all(builds.iter().map(|crate_type| async move { PENDING_BUILDS.lock().await.insert(*crate_type) })), join_all(copy_futures));
	} else {
		// crate-specific changes
		let builds = find_uniques(
			&event
				.paths
				.iter()
				.flat_map(|path| {
					let path_str = path.to_str().unwrap_or_default();
					ExtensionCrate::iter().filter(|e_crate| path_str.contains(&e_crate.get_crate_name(config))).collect::<Vec<ExtensionCrate>>()
				})
				.collect::<Vec<ExtensionCrate>>(),
		);

		// update UI for affected builds
		for crate_type in &builds {
			update_task_status(&crate_type.get_task_name(), BuildStatus::Pending).await;
		}

		tokio::join!(join_all(builds.iter().map(|e_crate| async move { PENDING_BUILDS.lock().await.insert(*e_crate) }),), join_all(copy_futures));
	}
}

async fn process_pending_events(config: &ExtConfig) {
	let builds = PENDING_BUILDS.lock().await.drain().collect::<Vec<_>>();
	let copies = PENDING_COPIES.lock().await.drain().collect::<Vec<_>>();

	if builds.is_empty() && copies.is_empty() {
		return;
	}

	for build in &builds {
		update_task_status(&build.get_task_name(), BuildStatus::InProgress).await;
	}

	let build_results = join_all(builds.iter().map(|crate_type| async move {
		let result = crate_type.build_crate(config).await;
		let status = if result.is_ok() { BuildStatus::Success } else { BuildStatus::Failed };

		update_task_status(&crate_type.get_task_name(), status).await;
		result
	}))
	.await;

	let copy_results = try_join_all(copies.into_iter().map(|e_file| e_file.copy_file_to_dist(config))).await;

	for result in build_results {
		if let Err(e) = result {
			error!("Error during build: {}", e);
		}
	}

	if let Err(e) = copy_results {
		error!("Error during copy: {}", e);
	}
}
