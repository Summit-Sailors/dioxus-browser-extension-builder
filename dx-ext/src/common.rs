use {
	crate::{LogLevel, efile::EFile, extcrate::ExtensionCrate},
	clap::{ArgAction, Args, ValueHint},
	dashmap::{DashMap, DashSet},
	ratatui::crossterm::event::{KeyCode, MouseEvent},
	serde::{Deserialize, Serialize},
	std::{
		path::PathBuf,
		sync::LazyLock,
		time::{Duration, Instant, SystemTime},
	},
};

pub(crate) static PENDING_BUILDS: LazyLock<DashSet<ExtensionCrate>> = LazyLock::new(DashSet::new);
pub(crate) static PENDING_COPIES: LazyLock<DashSet<EFile>> = LazyLock::new(DashSet::new);
pub(crate) static FILE_HASHES: LazyLock<DashMap<PathBuf, String>> = LazyLock::new(DashMap::new);
pub(crate) static FILE_TIMESTAMPS: LazyLock<DashMap<PathBuf, SystemTime>> = LazyLock::new(DashMap::new);

// task progress tracking
#[derive(PartialEq, Default)]
#[allow(dead_code)]
pub enum TaskProgress {
	#[default]
	NotStarted,
	InProgress(f64),
	Completed,
	Failed,
}

// history tracking
#[derive(Debug, Clone)]
pub struct TaskState {
	pub status: TaskStatus,
	pub start_time: Option<Instant>,
	pub end_time: Option<Instant>,
	pub progress: Option<f64>,
	pub weight: f64,
}

impl Default for TaskState {
	fn default() -> Self {
		Self { status: TaskStatus::Pending, start_time: None, end_time: None, progress: None, weight: 1.0 }
	}
}

#[derive(Debug, Clone)]
pub struct TaskStats {
	pub total: usize,
	pub pending: usize,
	pub in_progress: usize,
	pub completed: usize,
	pub failed: usize,
}

#[allow(dead_code)]
impl TaskStats {
	pub fn is_all_complete(&self) -> bool {
		self.pending == 0 && self.in_progress == 0
	}

	pub fn has_failures(&self) -> bool {
		self.failed > 0
	}

	pub fn completion_ratio(&self) -> f64 {
		if self.total == 0 { 0.0 } else { (self.completed + self.failed) as f64 / self.total as f64 }
	}
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum TaskStatus {
	#[default]
	Pending,
	InProgress,
	Success,
	Failed,
}

#[derive(Debug, Clone)]
pub enum BuildState {
	Idle,
	Running { progress: f64, start_time: Instant },
	Complete { duration: Duration },
	Failed { duration: Duration },
}

#[allow(dead_code)]
pub(crate) enum EXMessage {
	Keypress(KeyCode),
	Paste(String),
	Mouse(MouseEvent),
	Tick,
	BuildProgress(f64),
	UpdateTask(String, TaskStatus),
	LogMessage(LogLevel, String),
	TaskProgress(String, f64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum BuildMode {
	Development,
	Release,
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
#[serde(rename_all = "kebab-case")]
pub(crate) struct TomlConfig {
	pub extension_config: ExtConfigToml,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ExtConfigToml {
	pub assets_directory: String,
	pub background_script_index_name: String,
	pub content_script_index_name: String,
	pub extension_directory_name: String,
	pub popup_name: String,
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
