use {
	crate::common::{BuildMode, ExtConfig},
	anyhow::Result,
	std::{process::Stdio, time::Duration},
	tokio::{process::Command, time::sleep},
	tracing::{debug, error, info},
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

	pub async fn build_crate<F>(&self, config: &ExtConfig, progress_callback: F) -> Option<Result<()>>
	where
		F: Fn(f64) + Clone + Send + 'static,
	{
		let extension_dir = &config.extension_directory_name;
		let crate_name = self.get_crate_name(config);
		let progress_callback_clone = progress_callback.clone();

		// report initial progress
		progress_callback(0.0);

		// define build phases with weight factors
		let phases = [("checking dependencies", 0.1), ("compiling", 0.4), ("optimizing wasm", 0.3), ("generating assets", 0.2)];

		// build the command
		let mut args = vec![
			"build".to_owned(),
			"--no-pack".to_owned(),
			"--no-typescript".to_owned(),
			"--target".to_owned(),
			"web".to_owned(),
			"--out-dir".to_owned(),
			"../dist".to_owned(),
		];

		// add release flag if needed
		if matches!(config.build_mode, BuildMode::Release) {
			args.push("--release".to_owned());
		}

		// add the crate path
		args.push(format!("{}/{}", extension_dir, crate_name));

		// create the command but don't execute it yet
		let mut cmd = Command::new("wasm-pack");
		cmd.args(&args);

		// we'll use stdout to parse progress
		cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

		// launch the process
		let mut child = match cmd.spawn() {
			Ok(child) => child,
			Err(e) => {
				error!("failed to start wasm-pack: {} ", e);
				return Some(Err(anyhow::anyhow!("failed to start build process: {}", e)));
			},
		};

		// get stdout and stderr handles
		let stdout = match child.stdout.take() {
			Some(stdout) => stdout,
			None => {
				error!("failed to capture wasm-pack stdout");
				return Some(Err(anyhow::anyhow!("failed to capture build output")));
			},
		};

		// spawn a task to track progress through log parsing
		// this is a simplified approach since wasm-pack doesn't report progress natively
		let progress_handle = tokio::spawn(async move {
			use tokio::io::{AsyncBufReadExt, BufReader};

			let mut current_phase = 0;
			let mut last_progress = 0.0;
			let reader = BufReader::new(stdout);
			let mut lines = reader.lines();

			while let Ok(Some(line)) = lines.next_line().await {
				let line_lower = line.to_lowercase();

				// detect phase transitions
				for (i, (phase_name, _)) in phases.iter().enumerate() {
					if line_lower.contains(phase_name) && i > current_phase {
						current_phase = i;
						last_progress = phases.iter().take(current_phase).fold(0.0, |acc, (_, weight)| acc + weight);
						progress_callback(last_progress);
						debug!("build phase: {}", phase_name);
						break;
					}
				}

				// increment progress within the current phase
				if let Some((_, weight)) = phases.get(current_phase) {
					// simple heuristic: update progress based on lines processed
					last_progress += weight / 50.0; // assuming ~50 lines per phase
					last_progress = last_progress.min(phases.iter().take(current_phase + 1).fold(0.0, |acc, (_, w)| acc + w));
					progress_callback(last_progress);
				}

				// small delay to prevent too many UI updates
				sleep(Duration::from_millis(10)).await;
			}
		});

		// wait for the process to complete
		let status = match child.wait().await {
			Ok(status) => status,
			Err(e) => {
				error!("Failed to wait for wasm-pack: {}", e);
				progress_callback_clone(1.0);
				return Some(Err(anyhow::anyhow!("Build process failed: {}", e)));
			},
		};

		// ensure the progress tracking task completes
		let _ = progress_handle.await;
		progress_callback_clone(1.0);

		if status.success() {
			info!("[SUCCESS] wasm-pack build for {}", crate_name);
			Some(Ok(()))
		} else {
			error!("[FAIL] wasm-pack build for {} failed with status: {}", crate_name, status);
			Some(Err(anyhow::anyhow!("Build failed for {}", crate_name)))
		}
	}
}
