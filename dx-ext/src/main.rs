use clap::{Parser, Subcommand};
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
	tracing_subscriber::{
		FmtSubscriber,
		fmt::{format::Writer, time::FormatTime},
	},
};

static PENDING_BUILDS: LazyLock<Mutex<HashSet<ExtensionCrate>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static PENDING_COPIES: LazyLock<Mutex<HashSet<EFile>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display, strum_macros::AsRefStr)]
#[strum(serialize_all = "lowercase")]
enum ExtensionCrate {
	Popup,
	Background,
	Content,
}

impl ExtensionCrate {
	async fn build_crate(self) -> Result<()> {
		info!("Building {}...", self.as_ref());
		let status = Command::new("wasm-pack")
			.args(["build", "--no-pack", "--no-typescript", "--target", "web", "--out-dir", "../dist", format!("extension/{self}").as_ref()])
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.await
			.context("Failed to execute wasm-pack")?;
		if !status.success() {
			warn!("[FAIL] wasm-pack build for {} failed with status: {}", self, status);
		} else {
			info!("[SUCCESS] wasm-pack build for {}", self);
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display, strum_macros::AsRefStr)]
enum EFile {
	#[strum(serialize = "manifest.json")]
	Manifest,
	#[strum(serialize = "index.html")]
	IndexHtml,
	#[strum(serialize = "index.js")]
	IndexJs,
	#[strum(serialize = "background_index.js")]
	IndexBackgroundJs,
	#[strum(serialize = "content_index.js")]
	IndexContentJs,
	#[strum(serialize = "popup/assets")]
	Assets,
}

impl EFile {
	fn get_copy_src(&self) -> PathBuf {
		Path::new("./extension").join(self.to_string()).clone()
	}

	fn get_copy_dest(&self) -> PathBuf {
		Path::new("./extension/dist")
			.join(match self {
				Self::Assets => "assets".to_owned(),
				_ => self.to_string(),
			})
			.clone()
	}

	async fn copy_file_to_dist(self) -> Result<()> {
		info!("Copying {self}...");
		let status = Command::new("cp")
			.args(["-fr", self.get_copy_src().to_str().expect("get_copy_src"), self.get_copy_dest().to_str().expect("get_copy_dest")])
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.await
			.context("Failed to execute cp")?;
		if !status.success() {
			warn!("copy for {} failed with status: {}", self, status);
		} else {
			info!("[SUCCESS] copy for {}", self);
		}
		Ok(())
	}
}

struct CustomTime;

impl FormatTime for CustomTime {
	fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
		write!(w, "{}", chrono::Local::now().format("%m-%d %H:%M"))
	}
}

async fn hot_reload() -> Result<()> {
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
	for e_file in EFile::iter() {
		watcher
			.watch(&Path::new("./extension").join(e_file.as_ref()), RecursiveMode::NonRecursive)
			.with_context(|| format!("Failed to watch directory: {e_file}"))?;
	}
	for e_crate in ExtensionCrate::iter() {
		watcher
			.watch(&Path::new("./extension").join(Path::new(e_crate.as_ref()).join("src")), RecursiveMode::Recursive)
			.with_context(|| format!("Failed to watch directory: {e_crate}"))?;
	}
	info!("File watcher started. Press Ctrl+C to stop.");
	let watch_task = tokio::spawn(watch_loop(rx, cancel_token.clone()));
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
	/// Start the file watcher and build system
	Watch,
	/// Build all crates and copy files without watching
	Build,
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();

	FmtSubscriber::builder().with_max_level(Level::INFO).with_timer(CustomTime).init();

	match cli.command {
		Commands::Watch => {
			hot_reload().await?;
		},
		Commands::Build => {
			let (build_result, copy_result) = tokio::join!(
				try_join_all(ExtensionCrate::iter().map(|crate_type| crate_type.build_crate())),
				try_join_all(EFile::iter().map(|e_file| e_file.copy_file_to_dist()))
			);
			if let Err(e) = build_result {
				error!("Error during builds: {}", e);
			}
			if let Err(e) = copy_result {
				error!("Error during copy: {}", e);
			}
		},
	}

	Ok(())
}

async fn watch_loop(mut rx: mpsc::Receiver<Event>, cancel_token: CancellationToken) {
	let mut pending_events = tokio::time::interval(Duration::from_secs(1));
	loop {
		tokio::select! {
			_ = cancel_token.cancelled() => break,
			Some(event) = rx.recv() => {
							handle_event(&event).await;
							pending_events.reset();
			}
			_ = pending_events.tick() => {
				process_pending_events().await;
			}
		}
	}
}

async fn handle_event(event: &Event) {
	let copy_futures = find_uniques(
		&event
			.paths
			.iter()
			.flat_map(|path| EFile::iter().filter(|e_file| path.to_str().unwrap_or_default().contains(e_file.as_ref())).collect::<Vec<EFile>>())
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
						.flat_map(|path| ExtensionCrate::iter()
							.filter(|e_crate| path.to_str().unwrap_or_default().contains(e_crate.as_ref()))
							.collect::<Vec<ExtensionCrate>>())
						.collect::<Vec<ExtensionCrate>>(),
				)
				.into_iter()
				.map(|e_crate| async move { PENDING_BUILDS.lock().await.insert(e_crate) }),
			),
			join_all(copy_futures)
		);
	}
}

async fn process_pending_events() {
	let (build_result, copy_result) = tokio::join!(
		try_join_all(PENDING_BUILDS.lock().await.drain().map(ExtensionCrate::build_crate)),
		try_join_all(PENDING_COPIES.lock().await.drain().map(EFile::copy_file_to_dist))
	);
	if let Err(e) = build_result {
		error!("Error during builds: {}", e);
	}
	if let Err(e) = copy_result {
		error!("Error during copy: {}", e);
	}
}
