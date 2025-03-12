use {
	crate::{efile::EFile, extcrate::ExtensionCrate},
	clap::{ArgAction, Args, ValueHint},
	crossterm::event::KeyCode,
	serde::{Deserialize, Serialize},
	std::{
		collections::{HashMap, HashSet},
		path::PathBuf,
		sync::{Arc, LazyLock},
		time::{Duration, Instant, SystemTime},
	},
	tokio::sync::Mutex,
	tracing::{Event, Subscriber, field::Visit},
	tracing_subscriber::{Layer, registry::LookupSpan},
};

pub(crate) static PENDING_BUILDS: LazyLock<Mutex<HashSet<ExtensionCrate>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) static PENDING_COPIES: LazyLock<Mutex<HashSet<EFile>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) static FILE_HASHES: LazyLock<Mutex<HashMap<PathBuf, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
pub(crate) static FILE_TIMESTAMPS: LazyLock<Mutex<HashMap<PathBuf, SystemTime>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

// type alias for a logging callback function
pub(crate) type LogCallback = Arc<Mutex<dyn Fn(LogLevel, &str) + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
	Debug,
	Info,
	Warn,
	Error,
}

// custom layer for tracing (that will forward logs to TUI)
pub(crate) struct TUILogLayer {
	callback: LogCallback,
}

impl TUILogLayer {
	pub(crate) fn new(callback: LogCallback) -> Self {
		Self { callback }
	}
}

impl<S> Layer<S> for TUILogLayer
where
	S: Subscriber + for<'a> LookupSpan<'a>,
{
	fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
		// log message extraction
		let mut message = String::new();
		struct MessageVisitor<'a>(&'a mut String);

		impl<'a> Visit for MessageVisitor<'a> {
			fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
				if field.name() == "message" {
					self.0.push_str(&format!("{:?}", value));
				}
			}

			fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
				if field.name() == "message" {
					self.0.push_str(value);
				}
			}
		}

		event.record(&mut MessageVisitor(&mut message));

		let level = match *event.metadata().level() {
			tracing::Level::DEBUG => LogLevel::Debug,
			tracing::Level::INFO => LogLevel::Info,
			tracing::Level::WARN => LogLevel::Warn,
			tracing::Level::ERROR => LogLevel::Error,
			_ => LogLevel::Error,
		};

		// Send the log to the TUI via callback
		let callback = self.callback.clone();
		tokio::spawn(async move {
			let callback_guard = callback.lock().await;
			(callback_guard)(level, &message);
		});
	}
}

// task progress tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskProgress {
	NotStarted,
	Running(f64),
	Completed(Duration),
	Failed(Duration),
}

impl Default for TaskProgress {
	fn default() -> Self {
		Self::NotStarted
	}
}

// history tracking
#[derive(Debug, Clone)]
pub struct TaskState {
	pub status: BuildStatus,
	pub start_time: Option<Instant>,
	pub end_time: Option<Instant>,
	pub progress: Option<f64>,
}

impl Default for TaskState {
	fn default() -> Self {
		Self { status: BuildStatus::Pending, start_time: None, end_time: None, progress: None }
	}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BuildStatus {
	Pending,
	InProgress,
	Success,
	Failed,
}

#[derive(Debug, Clone)]
pub enum BuilState {
	Idle,
	Running { progress: f64, start_time: Instant },
	Complete { duration: Duration },
	Failed { duration: Duration },
}

pub(crate) enum EXMessage {
	Keypress(KeyCode),
	Tick,
	BuildProgress(f64),
	BuildComplete,
	BuildFailed,
	Exit,
	UpdateTask(String, BuildStatus),
	LogMessage(LogLevel, String),
	TaskProgress(String, f64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BuildMode {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ExtConfig {
	pub(crate) background_script_index_name: String,
	pub(crate) content_script_index_name: String,
	pub(crate) extension_directory_name: String,
	pub(crate) popup_name: String,
	pub(crate) assets_dir: String,
	pub(crate) build_mode: BuildMode,
}

// config struct that matches the TOML structure
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TomlConfig {
	#[serde(rename = "extension-config")]
	pub(crate) extension_config: ExtConfigToml,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ExtConfigToml {
	#[serde(rename = "assets-directory")]
	pub(crate) assets_directory: String,

	#[serde(rename = "background-script-index-name")]
	pub(crate) background_script_index_name: String,

	#[serde(rename = "content-script-index-name")]
	pub(crate) content_script_index_name: String,

	#[serde(rename = "extension-directory-name")]
	pub(crate) extension_directory_name: String,

	#[serde(rename = "popup-name")]
	pub(crate) popup_name: String,
}

// Configuration options for the Init command
#[derive(Args, Debug)]
pub(crate) struct InitOptions {
	/// Extension directory name
	#[arg(long, help = "Name of your extension directory", default_value = "extension", value_hint = ValueHint::DirPath)]
	pub(crate) extension_dir: String,

	/// Popup crate name
	#[arg(long, help = "Name of your popup crate", default_value = "popup")]
	pub(crate) popup_name: String,

	/// Background script entry point
	#[arg(long, help = "Name of your background script entry point", default_value = "background_index.js")]
	pub(crate) background_script: String,

	/// Content script entry point
	#[arg(long, help = "Name of your content script entry point", default_value = "content_index.js")]
	pub(crate) content_script: String,

	/// Assets directory
	#[arg(long, help = "Your assets directory relative to the extension directory", default_value = "popup/assets", value_hint = ValueHint::DirPath)]
	pub(crate) assets_dir: String,

	/// Force overwrite existing config file
	#[arg(short, long, help = "Force overwrite of existing config file", action = ArgAction::SetTrue)]
	pub(crate) force: bool,

	/// Interactive mode to collect configuration
	#[arg(short, long, help = "Interactive mode to collect configuration", action = ArgAction::SetTrue)]
	pub(crate) interactive: bool,
}
