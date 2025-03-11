use {
	crate::common::{BuildMode, ExtConfig},
	anyhow::{Context, Result},
	std::process::Stdio,
	tokio::process::Command,
	tracing::{info, warn},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum ExtensionCrate {
	Popup,
	Background,
	Content,
}

impl ExtensionCrate {
	// the actual crate name based on config
	pub(crate) fn get_crate_name(&self, config: &ExtConfig) -> String {
		match self {
			Self::Popup => config.popup_name.clone(),
			Self::Background => "background".to_owned(),
			Self::Content => "content".to_owned(),
		}
	}

	pub(crate) fn get_task_name(&self) -> String {
		match self {
			Self::Popup => "Building Popup".to_string(),
			Self::Background => "Building Background".to_string(),
			Self::Content => "Building Content".to_string(),
		}
	}

	pub(crate) async fn build_crate(self, config: &ExtConfig) -> Result<()> {
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
