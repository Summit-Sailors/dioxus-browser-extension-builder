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
//! - The needs_rebuild function checks if a rebuild is necessary based on file timestamps.
//! - The build_crate function runs wasm-pack build, tracking progress with a callback.
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
	logging::{LogCallback, LogLevel, TUILogLayer},
	lowdash::find_uniques,
	notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher},
	std::{io::stdout, path::Path, sync::Arc, time::Duration},
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
	utils::{clean_dist_directory, create_default_config_toml, read_config, show_final_build_report},
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

	match cli.command {
		Commands::Init(options) => {
			let subscriber = FmtSubscriber::builder().with_timer(CustomTime).with_max_level(Level::INFO).finish();
			let _ = tracing::subscriber::set_global_default(subscriber);

			create_default_config_toml(&options)?;
			println!("Created dx-ext.toml configuration file");
			return Ok(());
		},
		_ => {
			let (app, terminal, ui_rx, log_callback) = setup_tui().await?;
			let tui_layer = TUILogLayer::new(log_callback);
			let subscriber = tracing_subscriber::registry().with(tui_layer);
			let _ = tracing::subscriber::set_global_default(subscriber);

			let original_hook = std::panic::take_hook();
			std::panic::set_hook(Box::new(move |info| {
				let _ = disable_raw_mode();
				let _ = stdout().execute(Show);
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

					for e_crate in ExtensionCrate::iter() {
						update_task_status(&e_crate.get_task_name(), BuildStatus::InProgress).await;
						let result = e_crate
							.build_crate(&config, move |progress| {
								let task_name = e_crate.get_task_name();
								let _ = tokio::spawn(async move {
									send_ui_message(EXMessage::TaskProgress(task_name, progress)).await;
								});
							})
							.await;

						match result {
							Some(Ok(_)) => update_task_status(&e_crate.get_task_name(), BuildStatus::Success).await,
							Some(Err(e)) => {
								error!("Failed to build {}: {:?}", e_crate.get_task_name(), e);
								update_task_status(&e_crate.get_task_name(), BuildStatus::Failed).await;
							},
							None => todo!(),
						}
					}

					for e_file in EFile::iter() {
						if let Err(e) = e_file.copy_file_to_dist(&config).await {
							error!("Failed to copy file: {}", e);
						}
					}

					let _ = sleep(Duration::from_millis(500)).await; // wait for full UI update
					cancel_token.cancel();
					let _ = ui_task.await;
					show_final_build_report(app).await;
				},
				_ => unreachable!(),
			}
		},
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
		let message = EXMessage::LogMessage(level, msg.to_string());
		tokio::spawn(send_ui_message(message));
	}));

	let terminal = Arc::new(Mutex::new(Terminal::new()?));

	Ok((app, terminal, ui_rx, log_callback))
}

async fn update_task_status(task_name: &str, status: BuildStatus) {
	send_ui_message(EXMessage::UpdateTask(task_name.to_string(), status)).await;
}

async fn hot_reload(config: ExtConfig, app: Arc<Mutex<App>>, terminal: Arc<Mutex<Terminal>>, ui_rx: mpsc::UnboundedReceiver<EXMessage>) -> Result<()> {
	let app_clone = app.clone();
	let terminal_clone = terminal.clone();

	for e_crate in ExtensionCrate::iter() {
		app.lock().await.tasks.insert(e_crate.get_task_name(), BuildStatus::Pending);
	}

	let cancel_token = CancellationToken::new();
	let ui_cancel_token = cancel_token.clone();
	let watch_cancel_token = cancel_token.clone();

	// UI event loop
	let ui_task = tokio::spawn(run_ui_loop(app.clone(), terminal, ui_rx, ui_cancel_token));

	info!("Building extension crates...");
	for e_crate in ExtensionCrate::iter() {
		PENDING_BUILDS.lock().await.insert(e_crate);
		update_task_status(&e_crate.get_task_name(), BuildStatus::InProgress).await;

		let result = e_crate
			.build_crate(&config, move |progress| {
				let task_name = e_crate.get_task_name();
				let _ = tokio::spawn(async move {
					send_ui_message(EXMessage::TaskProgress(task_name, progress)).await;
				});
			})
			.await;

		match result {
			Some(Ok(_)) => {
				update_task_status(&e_crate.get_task_name(), BuildStatus::Success).await;
				PENDING_BUILDS.lock().await.remove(&e_crate);
			},
			Some(Err(e)) => {
				error!("Build failed for {}: {:?}", e_crate.get_task_name(), e);
				update_task_status(&e_crate.get_task_name(), BuildStatus::Failed).await;
			},
			None => {},
		}
	}

	for e_file in EFile::iter() {
		PENDING_COPIES.lock().await.insert(e_file);
		if let Err(e) = e_file.copy_file_to_dist(&config).await {
			error!("Failed to copy file: {}", e);
		} else {
			PENDING_COPIES.lock().await.remove(&e_file);
		}
	}

	info!("Initial build completed, setting up file watcher...");

	// now the watcher
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

	// a small delay to ensure watcher is ready
	sleep(Duration::from_millis(200)).await;

	// watch task
	let config_clone = config.clone();
	let watch_task = tokio::spawn(async move {
		watch_loop(rx, watch_cancel_token, config_clone, app_clone, terminal_clone).await;
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
	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => break,
			_ = interval.tick() => {
				// UI updates handler
				{
					let mut app = app.lock().await;
					app.update(EXMessage::Tick);
				}
				// UI draw
				{
					let mut app_guard = app.lock().await;
					if app_guard.should_quit {
						break;
					}
					let mut terminal_guard = terminal.lock().await;
					if let Err(e) = terminal_guard.draw(&mut app_guard) {
						error!("Failed to draw UI: {}", e);
						break;
					}
				}
				// checking for key events
				if crossterm::event::poll(Duration::from_millis(0))? {
					if let crossterm::event::Event::Key(key) = event::read()? {
						if key.kind == KeyEventKind::Press {
							if key.code == KeyCode::Char('r') {
								{
									let mut app_guard = app.lock().await;
									let _ = app_guard.update(EXMessage::Keypress(key.code));
									let mut terminal_guard = terminal.lock().await;
									if let Err(e) = terminal_guard.draw(&mut app_guard) {
										error!("Failed to draw UI: {}", e);
									}
								}

								send_ui_message(EXMessage::LogMessage(LogLevel::Info, "Cleaning dist directory...".to_string())).await;
								if let Err(e) = clean_dist_directory(&read_config()?).await {
									error!("Failed to clean dist directory: {}", e);
								}

								{
									let mut app_guard = app.lock().await;
									let mut terminal_guard = terminal.lock().await;
									if let Err(e) = terminal_guard.draw(&mut app_guard) {
										error!("Failed to draw UI: {}", e);
									}
								}

								send_ui_message(EXMessage::LogMessage(LogLevel::Info, "Reinitializing build tasks...".to_string())).await;
								for e_crate in ExtensionCrate::iter() {
									PENDING_BUILDS.lock().await.insert(e_crate);
									update_task_status(&e_crate.get_task_name(), BuildStatus::Pending).await;

									{
										let mut app_guard = app.lock().await;
										let mut terminal_guard = terminal.lock().await;
										if let Err(e) = terminal_guard.draw(&mut app_guard) {
											error!("Failed to draw UI: {}", e);
										}
									}
								}

								for e_file in EFile::iter() {
									PENDING_COPIES.lock().await.insert(e_file);
								}

								send_ui_message(EXMessage::LogMessage(LogLevel::Info, "Starting rebuild process...".to_string())).await;

								{
									let mut app_guard = app.lock().await;
									app_guard.update(EXMessage::BuildProgress(0.0));
									let mut terminal_guard = terminal.lock().await;
									if let Err(e) = terminal_guard.draw(&mut app_guard) {
											error!("Failed to draw UI: {}", e);
									}
							}
								process_pending_events(&read_config()?, app.clone(), terminal.clone()).await;
						}

							if key.code == KeyCode::Char('q') ||
								(key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)) {
								let mut app_guard = app.lock().await;
								app_guard.update(EXMessage::Exit);
							}
						}
					}
				}
			}
			Some(ui_msg) = ui_rx.recv() => {
				let mut app_guard = app.lock().await;
				app_guard.update(ui_msg);
				let mut terminal_guard = terminal.lock().await;
				if let Err(e) = terminal_guard.draw(&mut app_guard) {
						error!("Failed to draw UI: {}", e);
				}
			}
		}
	}
	Ok(())
}

async fn watch_loop(mut rx: mpsc::Receiver<Event>, cancel_token: CancellationToken, config: ExtConfig, app: Arc<Mutex<App>>, terminal: Arc<Mutex<Terminal>>) {
	let mut pending_events = tokio::time::interval(Duration::from_secs(1));

	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => break,
			Some(event) = rx.recv() => {
				handle_event(&event, &config).await;
				pending_events.reset();
			}
			_ = pending_events.tick() => {
				process_pending_events(&config, app.clone(), terminal.clone()).await;
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

async fn process_pending_events(config: &ExtConfig, app: Arc<Mutex<App>>, terminal: Arc<Mutex<Terminal>>) {
	let builds = PENDING_BUILDS.lock().await.drain().collect::<Vec<_>>();
	let copies = PENDING_COPIES.lock().await.drain().collect::<Vec<_>>();

	if builds.is_empty() && copies.is_empty() {
		return;
	}

	async fn redraw_ui(app: &Arc<Mutex<App>>, terminal: &Arc<Mutex<Terminal>>) {
		let mut app_guard = app.lock().await;
		let mut terminal_guard = terminal.lock().await;
		if let Err(e) = terminal_guard.draw(&mut app_guard) {
			error!("Failed to draw UI: {}", e);
		}
	}

	for build in &builds {
		update_task_status(&build.get_task_name(), BuildStatus::InProgress).await;
	}
	redraw_ui(&app, &terminal).await;

	let build_results = join_all(builds.iter().map(|crate_type| {
		let app_clone = Arc::clone(&app);
		let terminal_clone = Arc::clone(&terminal);
		let task_name = crate_type.get_task_name();

		async move {
			let task_name_clone = task_name.clone();

			send_ui_message(EXMessage::TaskProgress(task_name_clone.clone(), 0.0)).await;

			let progress_app = Arc::clone(&app_clone);
			let progress_terminal = Arc::clone(&terminal_clone);

			let result = crate_type
				.build_crate(config, move |progress| {
					let progress_task_name = task_name_clone.clone();
					let inner_app = Arc::clone(&progress_app);
					let inner_terminal = Arc::clone(&progress_terminal);

					let _ = tokio::spawn(async move {
						send_ui_message(EXMessage::TaskProgress(progress_task_name, progress)).await;

						{
							let mut app_guard = inner_app.lock().await;
							let mut terminal_guard = inner_terminal.lock().await;
							if let Err(e) = terminal_guard.draw(&mut app_guard) {
								error!("Failed to draw UI: {}", e);
							}
						}
					});
				})
				.await;

			let status = match &result {
				Some(Ok(_)) => {
					send_ui_message(EXMessage::TaskProgress(task_name.clone(), 1.0)).await;
					BuildStatus::Success
				},
				Some(Err(_)) | None => {
					send_ui_message(EXMessage::TaskProgress(task_name.clone(), 1.0)).await;
					BuildStatus::Failed
				},
			};

			update_task_status(&task_name, status).await;

			{
				let mut app_guard = app_clone.lock().await;
				let mut terminal_guard = terminal_clone.lock().await;
				if let Err(e) = terminal_guard.draw(&mut app_guard) {
					error!("Failed to draw UI: {}", e);
				}
			}

			match result {
				Some(r) => r,
				None => Err(anyhow::anyhow!("Build process failed for {}", task_name.clone())),
			}
		}
	}))
	.await;

	let copy_futures = copies.into_iter().map(|e_file| {
		let app = Arc::clone(&app);
		let terminal = Arc::clone(&terminal);

		async move {
			let result = e_file.copy_file_to_dist(config).await;
			redraw_ui(&app, &terminal).await;
			result
		}
	});

	let copy_results = try_join_all(copy_futures).await;

	for result in build_results {
		if let Err(e) = result {
			error!("Error during build: {}", e);
		}
	}

	if let Err(e) = copy_results {
		error!("Error during copy: {}", e);
	}

	for e_crate in ExtensionCrate::iter() {
		let task_name = e_crate.get_task_name();
		if let Some(status) = app.lock().await.tasks.get(&task_name) {
			if *status == BuildStatus::InProgress {
				update_task_status(&task_name, BuildStatus::Failed).await;
			}
		}
	}

	redraw_ui(&app, &terminal).await;
}
