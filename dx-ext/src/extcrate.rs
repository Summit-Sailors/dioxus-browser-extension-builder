use {
	crate::common::{BuildMode, ExtConfig},
	anyhow::Result,
	std::{fs, path::Path, process::Stdio, time::SystemTime},
	tokio::{
		io::{AsyncBufReadExt, BufReader},
		process::Command,
	},
	tracing::{debug, error, info, warn},
};

lazy_static::lazy_static!(
	static ref LOG_REGEX: regex::Regex = regex::Regex::new(r"\[INFO\]:|\[ERROR\]:|\[WARN\]:").expect("An error occurred when creating the Regex");
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum ExtensionCrate {
	Popup,
	Background,
	Content,
}

impl ExtensionCrate {
	// the actual crate name based on config
	pub fn get_crate_name(&self, config: &ExtConfig) -> String {
		match self {
			Self::Popup => config.popup_name.clone(),
			Self::Background => "background".to_owned(),
			Self::Content => "content".to_owned(),
		}
	}

	pub fn get_task_name(&self) -> String {
		match self {
			Self::Popup => "Building Popup".to_owned(),
			Self::Background => "Building Background".to_owned(),
			Self::Content => "Building Content".to_owned(),
		}
	}

	// check for crate-specific output files
	async fn needs_rebuild(crate_name: String, source_dir: String, target_dir: String) -> Result<bool> {
		let target_dir_path = Path::new(&target_dir);
		if !target_dir_path.exists() {
			return Ok(true);
		}
		let crate_output_js = target_dir_path.join(format!("{crate_name}_bg.js"));
		let crate_output_wasm = target_dir_path.join(format!("{crate_name}_bg.wasm"));
		if !crate_output_js.exists() || !crate_output_wasm.exists() {
			return Ok(true);
		}
		// oldest target file timestamps
		let oldest_target = [&crate_output_js, &crate_output_wasm]
			.iter()
			.filter_map(|path| fs::metadata(path).ok().and_then(|m| m.modified().ok()))
			.min()
			.unwrap_or_else(SystemTime::now);

		// find newest src file
		let source_dir_path = Path::new(&source_dir);
		if !source_dir_path.exists() {
			return Ok(true);
		}

		// tokio::task::spawn_blocking for CPU-bound file system ops
		let newest_source = tokio::task::spawn_blocking(move || {
			walkdir::WalkDir::new(source_dir)
				.min_depth(1)
				.into_iter()
				.filter_map(|entry| entry.ok())
				.filter(|e| e.file_type().is_file())
				.filter_map(|entry| fs::metadata(entry.path()).ok().and_then(|m| m.modified().ok()))
				.max()
				.unwrap_or(SystemTime::UNIX_EPOCH)
		})
		.await?;

		// if source is newer than target, rebuild is needed
		Ok(newest_source > oldest_target)
	}

	pub async fn build_crate<F>(&self, config: &ExtConfig, progress_callback: F) -> Option<Result<()>>
	where
		F: Fn(f64) + Clone + Send + 'static,
	{
		let extension_dir = &config.extension_directory_name;
		let crate_name = self.get_crate_name(config);
		let progress_callback_clone = progress_callback.clone();
		progress_callback(0.0);

		let should_build = if config.enable_incremental_builds {
			let source_dir = format!("{extension_dir}/{crate_name}");
			let target_dir = format!("{extension_dir}/dist");

			if !Path::new(&target_dir).exists() {
				if let Err(e) = fs::create_dir_all(&target_dir) {
					warn!("Failed to create target directory: {}", e);
				}
			}

			match Self::needs_rebuild(crate_name.clone(), source_dir.clone(), target_dir.clone()).await {
				Ok(true) => {
					debug!("Rebuild needed for {}", crate_name);
					true
				},
				Ok(false) => {
					info!("[SKIPPED] No changes detected for {}, skipping build", crate_name);
					progress_callback(1.0);
					false
				},
				Err(e) => {
					warn!("Failed to check if rebuild is needed: {}", e);
					true
				},
			}
		} else {
			true
		};

		if !should_build {
			return Some(Ok(()));
		}

		let mut attempts = 0;
		const MAX_ATTEMPTS: usize = 3;

		while attempts < MAX_ATTEMPTS {
			if attempts > 0 {
				progress_callback_clone(0.0);
			}

			let mut cmd = Command::new("wasm-pack");
			cmd.arg("build").arg("--no-pack").arg("--no-typescript").arg("--target").arg("web").arg("--out-dir").arg("../dist");

			if matches!(config.build_mode, BuildMode::Release) {
				cmd.arg("--release");
			}

			cmd.arg(format!("{extension_dir}/{crate_name}"));
			cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

			let mut child = match cmd.spawn() {
				Ok(child) => child,
				Err(e) => {
					error!("Failed to start wasm-pack: {}", e);
					if e.kind() == std::io::ErrorKind::NotFound {
						return Some(Err(anyhow::anyhow!("wasm-pack not found. Please install it with `cargo install wasm-pack`")));
					}
					return Some(Err(anyhow::anyhow!("Failed to start build process: {}", e)));
				},
			};

			let Some(_) = child.stdout.take() else {
				let _ = child.kill().await;
				error!("Failed to capture wasm-pack stdout");
				return Some(Err(anyhow::anyhow!("Failed to capture build output")));
			};

			let stderr = child.stderr.take();

			if let Some(stderr) = stderr {
				let _ = tokio::spawn(async move {
					let reader = BufReader::new(stderr);
					let mut lines = reader.lines();

					while let Ok(Some(line)) = lines.next_line().await {
						let clean_line = LOG_REGEX.replace_all(&line, "").trim().to_owned();

						if line.contains("[INFO]:") {
							info!("{}", clean_line);
						} else if line.contains("[ERROR]:") {
							error!("{}", clean_line);
						} else if line.contains("[WARN]:") {
							warn!("{}", clean_line);
						} else {
							debug!("{}", line);
						}
					}
				})
				.await;
			}

			// capture and stdout for better diagnostics
			if let Some(stdout) = child.stdout.take() {
				let crate_name_clone = crate_name.clone();
				tokio::spawn(async move {
					let reader = BufReader::new(stdout);
					let mut lines = reader.lines();

					while let Ok(Some(line)) = lines.next_line().await {
						debug!("[{}] {}", crate_name_clone, line);
					}
				});
			}

			match child.wait().await {
				Ok(status) if status.success() => {
					info!("wasm-pack build completed successfully for {}", crate_name);
					progress_callback(1.0);
					return Some(Ok(()));
				},
				Ok(_) => {
					error!("wasm-pack build failed for {}", crate_name);
					attempts += 1;
					if attempts < MAX_ATTEMPTS {
						warn!("Retrying build ({}/{})...", attempts, MAX_ATTEMPTS);
					}
				},
				Err(e) => {
					error!("Failed to wait for wasm-pack process: {}", e);
					attempts += 1;
					if attempts < MAX_ATTEMPTS {
						warn!("Retrying build ({}/{})...", attempts, MAX_ATTEMPTS);
					}
				},
			}
		}

		Some(Err(anyhow::anyhow!("Failed to build {} after {} attempts", crate_name, MAX_ATTEMPTS)))
	}
}
