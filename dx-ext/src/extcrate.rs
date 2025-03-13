use {
	crate::common::{BuildMode, ExtConfig},
	anyhow::Result,
	std::{
		fs,
		path::Path,
		process::Stdio,
		time::{Duration, SystemTime},
	},
	tokio::{process::Command, time::sleep},
	tracing::{debug, error, info, warn},
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

	async fn needs_rebuild(source_dir: &str, target_dir: &str) -> Result<bool> {
		// newest source file
		let mut newest_source = SystemTime::UNIX_EPOCH;
		let walker = walkdir::WalkDir::new(source_dir).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file());

		for entry in walker {
			if let Ok(metadata) = fs::metadata(entry.path()) {
				if let Ok(modified) = metadata.modified() {
					if modified > newest_source {
						newest_source = modified;
					}
				}
			}
		}

		// oldest target file
		let mut oldest_target = SystemTime::now();
		let walker = walkdir::WalkDir::new(target_dir).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file());

		let mut target_exists = false;

		for entry in walker {
			target_exists = true;
			if let Ok(metadata) = fs::metadata(entry.path()) {
				if let Ok(modified) = metadata.modified() {
					if modified < oldest_target {
						oldest_target = modified;
					}
				}
			}
		}

		// If no target exists or source is newer than target, rebuild is needed
		Ok(!target_exists || newest_source > oldest_target)
	}

	pub async fn build_crate<F>(&self, config: &ExtConfig, progress_callback: F) -> Option<Result<()>>
	where
		F: Fn(f64) + Clone + Send + 'static,
	{
		let extension_dir = &config.extension_directory_name;
		let crate_name = self.get_crate_name(config);
		let progress_callback_clone = progress_callback.clone();

		// report initial progress
		progress_callback(0.0);

		// check for the need to rebuild using target timestamps
		if config.enable_incremental_builds {
			let source_dir = format!("{}/{}", extension_dir, crate_name);
			let target_dir = format!("{}/dist", extension_dir);

			// skip rebuild if target is newer than all sources
			if Path::new(&target_dir).exists() {
				match Self::needs_rebuild(&source_dir, &target_dir).await {
					Ok(false) => {
						info!("[SKIPPED] No changes detected for {}, skipping build", crate_name);
						progress_callback(1.0);
						return Some(Ok(()));
					},
					Ok(true) => debug!("Rebuild needed for {}", crate_name),
					Err(e) => warn!("Failed to check if rebuild is needed: {}", e),
				}
			}
		}

		// command with builder pattern for better readability
		let mut cmd = Command::new("wasm-pack");
		cmd.arg("build").arg("--no-pack").arg("--no-typescript").arg("--target").arg("web").arg("--out-dir").arg("../dist");

		if matches!(config.build_mode, BuildMode::Release) {
			cmd.arg("--release");
		}

		// add the crate path
		cmd.arg(format!("{}/{}", extension_dir, crate_name));

		// stdout/stderr capture
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

		let stdout = match child.stdout.take() {
			Some(stdout) => stdout,
			None => {
				// try to kill the process if we can't capture output
				let _ = child.kill().await;
				error!("Failed to capture wasm-pack stdout");
				return Some(Err(anyhow::anyhow!("Failed to capture build output")));
			},
		};

		let stderr = child.stderr.take();

		let progress_handle = tokio::spawn(async move {
			use regex::Regex;
			use tokio::io::{AsyncBufReadExt, BufReader};

			let reader = BufReader::new(stdout);
			let mut lines = reader.lines();

			// more precise phase detection with regex
			let phase_patterns = [
				(Regex::new(r"(?i)checking").unwrap(), "checking dependencies", 0.1),
				(Regex::new(r"(?i)compiling").unwrap(), "compiling", 0.4),
				(Regex::new(r"(?i)optimizing").unwrap(), "optimizing wasm", 0.3),
				(Regex::new(r"(?i)generating|packaging").unwrap(), "generating assets", 0.2),
			];

			let mut current_phase = 0;
			let mut last_progress: f64;
			let mut phase_line_count = 0;

			let mut compiled_items = 0;
			let mut total_items_estimate = 50;

			while let Ok(Some(line)) = lines.next_line().await {
				if line.contains("Compiling") {
					compiled_items += 1;
					// adjust estimate based on what we're seeing
					if compiled_items > total_items_estimate / 2 {
						total_items_estimate = compiled_items * 2;
					}
				}

				// phase transitions detection
				for (i, (pattern, name, _weight)) in phase_patterns.iter().enumerate() {
					if pattern.is_match(&line) && i > current_phase {
						current_phase = i;
						last_progress = phase_patterns.iter().take(current_phase).fold(0.0, |acc, (_, _, w)| acc + w);
						progress_callback(last_progress);
						debug!("build phase: {}", name);
						phase_line_count = 0;
						break;
					}
				}

				if let Some((_, _, weight)) = phase_patterns.get(current_phase) {
					phase_line_count += 1;

					// diff progress calculation strategies per phase
					let phase_progress = match current_phase {
						1 => (compiled_items as f64 / total_items_estimate as f64).min(0.95), // compiling
						_ => (phase_line_count as f64 / 25.0).min(0.95),                      // other phases
					};

					last_progress = phase_patterns.iter().take(current_phase).fold(0.0, |acc, (_, _, w)| acc + w) + (weight * phase_progress);

					progress_callback(last_progress);
				}

				// avoid flooding with progress updates
				sleep(Duration::from_millis(5)).await;
			}
		});

		let status = match child.wait().await {
			Ok(status) => status,
			Err(e) => {
				error!("Failed to wait for wasm-pack: {}", e);
				progress_callback_clone(1.0);
				return Some(Err(anyhow::anyhow!("Build process failed: {}", e)));
			},
		};

		let _ = progress_handle.await;
		progress_callback_clone(1.0);

		if !status.success() {
			if let Some(stderr) = stderr {
				use tokio::io::{AsyncBufReadExt, BufReader};

				let reader = BufReader::new(stderr);
				let mut lines = reader.lines();
				let mut error_output = String::with_capacity(4096);
				let mut line_count = 0;
				const MAX_ERROR_LINES: usize = 100;

				while let Ok(Some(line)) = lines.next_line().await {
					error_output.push_str(&line);
					error_output.push('\n');

					line_count += 1;
					if line_count >= MAX_ERROR_LINES {
						error_output.push_str("[... additional error output truncated ...]");
						break;
					}
				}

				if !error_output.is_empty() {
					error!("[FAIL] wasm-pack build for {} failed with errors:\n{}", crate_name, error_output);
					return Some(Err(anyhow::anyhow!("Build failed for {}: {}", crate_name, error_output)));
				}
			}

			error!("[FAIL] wasm-pack build for {} failed with status: {}", crate_name, status);
			return Some(Err(anyhow::anyhow!("Build failed for {}", crate_name)));
		} else {
			info!("[SUCCESS] wasm-pack build for {}", crate_name);
			Some(Ok(()))
		}
	}
}
