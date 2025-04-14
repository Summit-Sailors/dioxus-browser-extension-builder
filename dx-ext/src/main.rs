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
//! enable-incremental-builds = false                    # enable incremental builds for watch command
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
//!
//! Build operations for crates are managed through the `ExtensionCrate` enum which uses `wasm-pack`:
//! - It represents different browser extension components: Popup, Background, and Content.
//! - It provides methods to get the crate name and task name for each component.
//! - The `needs_rebuild` function checks if a rebuild is necessary based on file timestamps.
//! - The `build_crate` function runs wasm-pack build, tracking progress with a callback.
//! - It includes error handling, incremental builds, and phase-based progress estimation.

mod app;
mod common;
mod efile;
mod extcrate;
mod logging;
mod terminal;
mod utils;

use {
	anyhow::{Context, Result},
	app::App,
	clap::{ArgAction, Args, Parser, Subcommand},
	common::{BuildMode, BuildStatus, EXMessage, ExtConfig, InitOptions, PENDING_BUILDS, PENDING_COPIES},
	crossterm::event::{self, KeyCode, KeyEventKind},
	efile::EFile,
	extcrate::ExtensionCrate,
	futures::future::{join_all, try_join_all},
	lazy_static::lazy_static,
	logging::{LogCallback, LogLevel, TUILogLayer},
	notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher},
	std::{path::Path, sync::Arc, time::Duration},
	strum::IntoEnumIterator,
	terminal::Terminal,
	tokio::{
		sync::{Mutex, mpsc},
		time::sleep,
	},
	tokio_util::sync::CancellationToken,
	tracing::{Level, error, info, warn},
	tracing_subscriber::{
		FmtSubscriber,
		fmt::{format::Writer, time::FormatTime},
		layer::SubscriberExt,
	},
	utils::{clean_dist_directory, create_default_config_toml, read_config, setup_project_from_config, show_final_build_report},
};

lazy_static! {
	pub(crate) static ref UI_SENDER: Mutex<Option<mpsc::UnboundedSender<EXMessage>>> = Mutex::new(None);
}

const TICK_RATE_MS: u64 = 100;

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

struct CustomTime;

impl FormatTime for CustomTime {
	fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
		write!(w, "{}", chrono::Local::now().format("%m-%d %H:%M"))
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	if let Commands::Init(options) = cli.command {
		let subscriber = FmtSubscriber::builder().with_timer(CustomTime).with_max_level(Level::INFO).with_file(false).with_target(false).finish();
		let _ = tracing::subscriber::set_global_default(subscriber);

		let created = create_default_config_toml(&options)?;
		if created {
			info!("Created dx-ext.toml configuration file");
			let _ = setup_project_from_config();
		}
		return Ok(());
	} else {
		let (app, terminal, ui_rx, log_callback) = setup_tui().await?;
		let tui_layer = TUILogLayer::new(log_callback);
		let log_level = match &cli.command {
			Commands::Watch(options) | Commands::Build(options) => match options.mode {
				BuildMode::Development => Level::DEBUG,
				BuildMode::Release => Level::INFO,
			},
			Commands::Init(_) => Level::INFO,
		};
		let subscriber = tracing_subscriber::registry().with(tui_layer).with(tracing_subscriber::filter::LevelFilter::from_level(log_level));
		let _ = tracing::subscriber::set_global_default(subscriber);
		let original_hook = std::panic::take_hook();
		let terminal_clone = terminal.clone();
		std::panic::set_hook(Box::new(move |info| {
			terminal_clone.clone().blocking_lock().leave();
			original_hook(info);
		}));

		match cli.command {
			Commands::Watch(options) => {
				let mut config = read_config().context("Failed to read configuration")?;
				config.build_mode = options.mode;
				info!("Using extension directory: {}", config.extension_directory_name);
				if options.clean {
					clean_dist_directory(&config).await?;
				}
				hot_reload(config, app, terminal, ui_rx).await?;
			},
			Commands::Build(options) => {
				let mut config = read_config().context("Failed to read configuration")?;
				config.build_mode = options.mode;
				info!("Using extension directory: {}", config.extension_directory_name);
				if options.clean {
					clean_dist_directory(&config).await?;
				}
				let cancel_token = CancellationToken::new();
				let ui_task = tokio::spawn(run_ui_loop(app.clone(), terminal, ui_rx, cancel_token.clone()));
				// build all crates concurrently
				let build_futures = ExtensionCrate::iter().map(|e_crate| {
					let config = config.clone();
					let task_name = e_crate.get_task_name();
					let task_name_clone = task_name.clone();
					async move {
						update_task_status(&task_name, BuildStatus::InProgress).await;
						let progress_callback = move |progress| {
							let task = task_name.clone();
							tokio::spawn(async move {
								send_ui_message(EXMessage::TaskProgress(task, progress)).await;
							});
						};
						let result = e_crate.build_crate(&config, progress_callback).await;
						let status = match &result {
							Some(Ok(_)) => BuildStatus::Success,
							Some(Err(e)) => {
								error!("Failed to build {}: {:?}", e_crate.get_task_name(), e);
								BuildStatus::Failed
							},
							None => BuildStatus::Failed,
						};

						update_task_status(&task_name_clone, status).await;
						result
					}
				});
				join_all(build_futures).await;

				let copy_futures = EFile::iter().map(|e_file| {
					let config = config.clone();
					async move {
						if let Err(e) = e_file.copy_file_to_dist(&config).await {
							error!("Failed to copy file: {}", e);
						}
					}
				});
				join_all(copy_futures).await;
				let _ = sleep(Duration::from_millis(500)).await; // wait for full UI update
				cancel_token.cancel();
				let _ = ui_task.await;
				show_final_build_report(app).await;
			},
			Commands::Init(_) => unreachable!(),
		}
	}
	Ok(())
}

async fn initialize_sender() -> mpsc::UnboundedReceiver<EXMessage> {
	let (tx, rx) = mpsc::unbounded_channel();
	let mut sender = UI_SENDER.lock().await;
	*sender = Some(tx);
	rx
}

async fn send_ui_message(message: EXMessage) {
	let sender = UI_SENDER.lock().await;
	if let Some(tx) = sender.as_ref() {
		if let Err(e) = tx.send(message) {
			error!("Error sending message: {}", e);
		}
	} else {
		error!("Sender not initialized");
	}
}

async fn setup_tui() -> Result<(Arc<Mutex<App>>, Arc<Mutex<Terminal>>, mpsc::UnboundedReceiver<EXMessage>, LogCallback)> {
	let app = Arc::new(Mutex::new(App::new()));
	let ui_rx = initialize_sender().await;

	let log_callback = Arc::new(Mutex::new(move |level: LogLevel, msg: &str| {
		let message = EXMessage::LogMessage(level, msg.to_owned());
		tokio::spawn(send_ui_message(message));
	}));

	let terminal = Arc::new(Mutex::new(Terminal::new()?));

	Ok((app, terminal, ui_rx, log_callback))
}

async fn update_task_status(task_name: &str, status: BuildStatus) {
	send_ui_message(EXMessage::UpdateTask(task_name.to_owned(), status)).await;
}

async fn hot_reload(config: ExtConfig, app: Arc<Mutex<App>>, terminal: Arc<Mutex<Terminal>>, ui_rx: mpsc::UnboundedReceiver<EXMessage>) -> Result<()> {
	let cancel_token = CancellationToken::new();
	let ext_dir_binding = format!("./{}", config.extension_directory_name);
	let ext_dir = Path::new(&ext_dir_binding);
	let app_clone = app.clone();
	{
		let mut app_guard = app.lock().await;
		for e_crate in ExtensionCrate::iter() {
			app_guard.tasks.insert(e_crate.get_task_name(), BuildStatus::Pending);
		}
	}
	let ui_task = tokio::spawn(run_ui_loop(app.clone(), terminal, ui_rx, cancel_token.clone()));
	info!("Building extension crates...");
	let build_futures = ExtensionCrate::iter().map(|e_crate| {
		let config = config.clone();
		let task_name = e_crate.get_task_name();
		let task_name_clone = task_name.clone();
		async move {
			update_task_status(&task_name, BuildStatus::InProgress).await;
			let progress_callback = move |progress| {
				let task = task_name.clone();
				tokio::spawn(async move {
					send_ui_message(EXMessage::TaskProgress(task, progress)).await;
				});
			};
			let result = e_crate.build_crate(&config, progress_callback).await;
			let status = match &result {
				Some(Ok(_)) => BuildStatus::Success,
				Some(Err(e)) => {
					error!("Failed to build {}: {:?}", e_crate.get_task_name(), e);
					BuildStatus::Failed
				},
				None => BuildStatus::Failed,
			};
			update_task_status(&task_name_clone, status).await;
			result
		}
	});
	join_all(build_futures).await;

	let copy_futures = EFile::iter().map(|e_file| {
		let config = config.clone();
		async move {
			PENDING_COPIES.lock().await.insert(e_file);
			let result = e_file.copy_file_to_dist(&config).await;
			if let Err(e) = &result {
				error!("Failed to copy file: {}", e);
			} else {
				PENDING_COPIES.lock().await.remove(&e_file);
			}
			result
		}
	});
	join_all(copy_futures).await;
	info!("Initial build completed, setting up file watcher...");
	let (tx, rx) = mpsc::channel(100);
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

	let watch_task = tokio::spawn({
		let cancel_token = cancel_token.clone();
		async move {
			watch_loop(rx, cancel_token, config.clone(), app_clone).await;
		}
	});

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
	terminal: Arc<Mutex<Terminal>>,
	mut ui_rx: mpsc::UnboundedReceiver<EXMessage>,
	cancel_token: CancellationToken,
) -> Result<()> {
	let mut interval = tokio::time::interval(Duration::from_millis(TICK_RATE_MS));
	// pre-check for key events we care about
	let key_event_filter = |key: &KeyCode| -> bool { matches!(key, KeyCode::Char('q' | 'r') | KeyCode::Up | KeyCode::Down) };
	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => {
				terminal.lock().await.leave();
				break;
			},
			_ = interval.tick() => {
				let should_quit = {
					let mut app = app.lock().await;
				app.update(EXMessage::Tick).await;

				// poll for key events with 0 timeout
				if event::poll(Duration::from_millis(0))? {
					if let event::Event::Key(key) = event::read()? {
						if key.kind == KeyEventKind::Press && key_event_filter(&key.code) {
							app.update(EXMessage::Keypress(key.code)).await;
						}
					}
				}
				let should_quit = app.should_quit;
				// UI draw if not quitting
				if !should_quit {
					let mut terminal_guard = terminal.lock().await;
					if let Err(e) = terminal_guard.draw(&mut app) {
						error!("Failed to draw UI: {}", e);
						return Err(e.into());
					}
				}
				should_quit
				};
				if should_quit {
					terminal.lock().await.leave();
					break;
				}
			}
			Some(ui_msg) = ui_rx.recv() => {
				let mut app_guard = app.lock().await;
				app_guard.update(ui_msg).await;
				let mut terminal_guard = terminal.lock().await;
				if app_guard.should_quit {
					terminal_guard.leave();
					break;
				}
				if let Err(e) = terminal_guard.draw(&mut app_guard) {
					error!("Failed to draw UI: {}", e);
					return Err(e.into());
				}
			}
		}
	}
	Ok(())
}

async fn watch_loop(mut rx: mpsc::Receiver<Event>, cancel_token: CancellationToken, config: ExtConfig, app: Arc<Mutex<App>>) {
	let mut pending_events = tokio::time::interval(Duration::from_secs(1));

	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => break,
			Some(event) = rx.recv() => {
				app.lock().await.overall_start_time = None;
				handle_event(&event, &config).await;
				pending_events.reset();
			}
			_ = pending_events.tick() => {
				process_pending_events(&config, app.clone()).await;
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

	let mut pending_copies = PENDING_COPIES.lock().await;
	let copy_futures = event
		.paths
		.iter()
		.flat_map(|path| {
			let path_str = path.to_str().unwrap_or_default();
			EFile::iter().filter(|e_file| path_str.contains(&e_file.get_watch_path(config)))
		})
		.collect::<Vec<_>>();

	if !copy_futures.is_empty() {
		pending_copies.extend(copy_futures);
	}

	let mut pending_builds = PENDING_BUILDS.lock().await;
	if event.paths.iter().any(|path| path.to_str().unwrap_or_default().contains("api")) {
		pending_builds.extend(ExtensionCrate::iter());
	} else {
		let builds: Vec<_> = event
			.paths
			.iter()
			.flat_map(|path| {
				let path_str = path.to_str().unwrap_or_default();
				ExtensionCrate::iter().filter(move |e_crate| path_str.contains(&e_crate.get_crate_name(config)))
			})
			.collect();

		if !builds.is_empty() {
			for crate_type in &builds {
				update_task_status(&crate_type.get_task_name(), BuildStatus::Pending).await;
			}
			pending_builds.extend(builds);
		}
	}
}

async fn process_pending_events(config: &ExtConfig, app: Arc<Mutex<App>>) {
	let builds = {
		let mut pending_builds = PENDING_BUILDS.lock().await;
		if pending_builds.is_empty() { Vec::new() } else { pending_builds.drain().collect() }
	};
	let copies = {
		let mut pending_copies = PENDING_COPIES.lock().await;
		if pending_copies.is_empty() { Vec::new() } else { pending_copies.drain().collect() }
	};

	if builds.is_empty() && copies.is_empty() {
		return;
	}

	if !builds.is_empty() {
		let task_names: Vec<String> = builds.iter().map(|build| build.get_task_name()).collect();
		let update_futures = task_names.iter().map(|task_name| update_task_status(task_name, BuildStatus::InProgress));
		join_all(update_futures).await;
	}

	let build_results = join_all(builds.iter().map(|crate_type| {
		let task_name = crate_type.get_task_name();
		async move {
			let task_name_clone = task_name.clone();
			// progress reporting callback
			let progress_callback = move |progress| {
				let progress_task_name = task_name_clone.clone();
				tokio::spawn(async move {
					send_ui_message(EXMessage::TaskProgress(progress_task_name, progress)).await;
				});
			};
			let result = crate_type.build_crate(config, progress_callback).await;
			let status = match &result {
				Some(Ok(_)) => BuildStatus::Success,
				_ => BuildStatus::Failed,
			};
			update_task_status(&task_name, status).await;
			info!("{} completed with status: {:?}", task_name, status);
			result.unwrap_or_else(|| Err(anyhow::anyhow!("Build process failed for {}", task_name.clone())))
		}
	}))
	.await;

	if !copies.is_empty() {
		let copy_futures = copies.into_iter().map(|e_file| e_file.copy_file_to_dist(config));
		let copy_results = try_join_all(copy_futures).await;
		if let Err(e) = copy_results {
			error!("Error during copy: {}", e);
		}
	}

	// report build errors
	for result in build_results {
		if let Err(e) = result {
			error!("Error during build: {}", e);
		}
	}
	// final task statuses
	let mut app_lock = app.lock().await;
	for e_crate in ExtensionCrate::iter() {
		let task_name = e_crate.get_task_name();
		if let Some(status) = app_lock.tasks.get_mut(&task_name) {
			if *status == BuildStatus::InProgress {
				*status = BuildStatus::Failed;
				info!("Finalizing {}...", task_name);
			}
		}
	}
}
