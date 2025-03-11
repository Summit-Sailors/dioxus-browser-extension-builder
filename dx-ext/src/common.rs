use {
	crate::{efile::EFile, extcrate::ExtensionCrate},
	clap::{ArgAction, Args, ValueHint},
	crossterm::event::KeyCode,
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
	Failed,
}

#[derive(Debug, Clone)]
pub enum EXMessage {
	Tick,
	Keypress(KeyCode),
	BuildProgress(f64),
	BuildComplete,
	BuildFailed,
	Exit,
	UpdateTask(String, BuildStatus),
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
