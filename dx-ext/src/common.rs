use {
	crate::{LogLevel, efile::EFile, extcrate::ExtensionCrate},
	clap::{ArgAction, Args, ValueHint},
	serde::{Deserialize, Serialize},
	std::{
		collections::{HashMap, HashSet},
		path::PathBuf,
		sync::LazyLock,
		time::{Duration, Instant, SystemTime},
	},
	tokio::sync::Mutex,
};

pub(crate) static PENDING_BUILDS: LazyLock<Mutex<HashSet<ExtensionCrate>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) static PENDING_COPIES: LazyLock<Mutex<HashSet<EFile>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) static FILE_HASHES: LazyLock<Mutex<HashMap<PathBuf, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
pub(crate) static FILE_TIMESTAMPS: LazyLock<Mutex<HashMap<PathBuf, SystemTime>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

// task progress tracking
#[derive(PartialEq)]
#[allow(dead_code)]
pub enum TaskProgress {
	NotStarted,
	InProgress(f64),
	Completed,
	Failed,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

#[allow(dead_code)]
pub(crate) enum EXMessage {
	Keypress(ratatui::crossterm::event::KeyCode),
	Tick,
	BuildProgress(f64),
	BuildComplete,
	BuildFailed,
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
	pub background_script_index_name: String,
	pub content_script_index_name: String,
	pub extension_directory_name: String,
	pub popup_name: String,
	pub assets_dir: String,
	pub build_mode: BuildMode,
	pub enable_incremental_builds: bool,
}

// config struct that matches the TOML structure
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TomlConfig {
	#[serde(rename = "extension-config")]
	pub extension_config: ExtConfigToml,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ExtConfigToml {
	#[serde(rename = "assets-directory")]
	pub assets_directory: String,

	#[serde(rename = "background-script-index-name")]
	pub background_script_index_name: String,

	#[serde(rename = "content-script-index-name")]
	pub content_script_index_name: String,

	#[serde(rename = "extension-directory-name")]
	pub extension_directory_name: String,

	#[serde(rename = "popup-name")]
	pub popup_name: String,

	#[serde(rename = "enable-incremental-builds")]
	pub enable_incremental_builds: bool,
}

// Configuration options for the Init command
#[derive(Args, Debug)]
pub(crate) struct InitOptions {
	/// Extension directory name
	#[arg(long, help = "Name of your extension directory", default_value = "extension", value_hint = ValueHint::DirPath)]
	pub extension_dir: String,

	/// Popup crate name
	#[arg(long, help = "Name of your popup crate", default_value = "popup")]
	pub popup_name: String,

	/// Background script entry point
	#[arg(long, help = "Name of your background script entry point", default_value = "background_index.js")]
	pub background_script: String,

	/// Content script entry point
	#[arg(long, help = "Name of your content script entry point", default_value = "content_index.js")]
	pub content_script: String,

	/// Assets directory
	#[arg(long, help = "Your assets directory relative to the extension directory", default_value = "popup/assets", value_hint = ValueHint::DirPath)]
	pub assets_dir: String,

	/// Force overwrite existing config file
	#[arg(short, long, help = "Force overwrite of existing config file", action = ArgAction::SetTrue)]
	pub force: bool,

	/// Interactive mode to collect configuration
	#[arg(short, long, help = "Interactive mode to collect configuration", action = ArgAction::SetTrue)]
	pub interactive: bool,

	/// Enable incremental build
	#[arg(short, long, help = "Enable incremental builds for watch command", action = ArgAction::SetTrue)]
	pub enable_incremental_builds: bool,
}
