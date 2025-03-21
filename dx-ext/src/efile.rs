use {
	crate::common::{ExtConfig, FILE_HASHES, FILE_TIMESTAMPS},
	anyhow::{Context, Result},
	futures::future::try_join_all,
	std::{
		fs,
		io::{self, Read},
		path::{Path, PathBuf},
	},
	tracing::{debug, info, warn},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::EnumIter, strum_macros::Display)]
pub(crate) enum EFile {
	// fixed files for Chrome extensions
	Manifest,
	IndexHtml,
	IndexJs,
	// dynamic files from config
	BackgroundScript,
	ContentScript,
	Assets,
}

impl EFile {
	fn get_copy_src(&self, config: &ExtConfig) -> PathBuf {
		let base_path_binding = format!("./{}", config.extension_directory_name);
		let base_path = Path::new(&base_path_binding);
		match self {
			Self::Manifest => base_path.join("manifest.json"),
			Self::IndexHtml => base_path.join("index.html"),
			Self::IndexJs => base_path.join("index.js"),
			Self::BackgroundScript => base_path.join(&config.background_script_index_name),
			Self::ContentScript => base_path.join(&config.content_script_index_name),
			Self::Assets => base_path.join(&config.assets_dir),
		}
	}

	fn get_copy_dest(&self, config: &ExtConfig) -> PathBuf {
		let dist_path_binding = format!("./{}/dist", config.extension_directory_name);
		let dist_path = Path::new(&dist_path_binding);
		match self {
			Self::Manifest => dist_path.join("manifest.json"),
			Self::IndexHtml => dist_path.join("index.html"),
			Self::IndexJs => dist_path.join("index.js"),
			Self::BackgroundScript => dist_path.join(&config.background_script_index_name),
			Self::ContentScript => dist_path.join(&config.content_script_index_name),
			Self::Assets => dist_path.join("assets"),
		}
	}

	fn calculate_file_hash(path: &Path) -> Result<String> {
		let file = fs::File::open(path).with_context(|| format!("Failed to open file for hashing: {path:?}"))?;
		let mut reader = io::BufReader::new(file);
		let mut hasher = blake3::Hasher::new();

		let mut buffer = [0; 65536];
		loop {
			let bytes_read = reader.read(&mut buffer).with_context(|| format!("Failed to read file for hashing: {path:?}"))?;

			if bytes_read == 0 {
				break;
			}

			hasher.update(&buffer[..bytes_read]);
		}

		Ok(hasher.finalize().to_hex().to_string())
	}

	async fn needs_copy(src: &Path, dest: &Path, src_metadata: &fs::Metadata) -> Result<bool> {
		let src_len = src_metadata.len();
		let src_time = src_metadata.modified().ok();

		if !dest.exists() {
			return Ok(true);
		}

		let dest_metadata = match fs::metadata(dest) {
			Ok(meta) => meta,
			Err(_) => return Ok(true), // if we can't read dest metadata, assume copy needed
		};

		// if sizes differ, definitely needs copy
		if src_len != dest_metadata.len() {
			return Ok(true);
		}

		// timestamps checks
		let timestamps = FILE_TIMESTAMPS.lock().await;
		if let Some(src_time) = src_time {
			if let Some(stored_time) = timestamps.get(src) {
				if *stored_time == src_time {
					// file hasn't changed since last check
					return Ok(false);
				}
			}
		}
		drop(timestamps); // release lock before potentially lengthy hash calculation

		// hashes comparison for final determination
		let src_hash = Self::calculate_file_hash(src)?;
		let dest_hash = Self::calculate_file_hash(dest)?;

		// cache update with single lock acquisition
		let (mut hashes, mut timestamps) = futures::join!(FILE_HASHES.lock(), FILE_TIMESTAMPS.lock());

		hashes.insert(src.to_path_buf(), src_hash.clone());
		if let Some(src_time) = src_time {
			timestamps.insert(src.to_path_buf(), src_time);
		}

		Ok(src_hash != dest_hash)
	}

	// hash checking to avoid unnecessary copies
	async fn copy_file(src: &Path, dest: &Path) -> Result<bool> {
		if !src.exists() {
			return Err(anyhow::anyhow!("Source file does not exist: {:?}", src));
		}

		let src_metadata = fs::metadata(src).with_context(|| format!("Failed to get metadata for source file: {src:?}"))?;

		let copy_needed = Self::needs_copy(src, dest, &src_metadata).await?;

		if copy_needed {
			// create parent directories if they don't exist
			if let Some(parent) = dest.parent() {
				fs::create_dir_all(parent).with_context(|| format!("Failed to create parent directory: {parent:?}"))?;
			}

			fs::copy(src, dest).with_context(|| format!("Failed to copy file from {src:?} to {dest:?}"))?;

			debug!("Copied file: {:?} -> {:?}", src, dest);
			Ok(true)
		} else {
			debug!("Skipped copying unchanged file: {:?}", src);
			Ok(false)
		}
	}

	// directory copy with parallel processing and hash checking
	async fn copy_dir_all(src: &Path, dst: &Path) -> Result<bool> {
		if !src.exists() {
			return Err(anyhow::anyhow!("Source directory does not exist: {:?}", src));
		}

		fs::create_dir_all(dst).with_context(|| format!("Failed to create destination directory: {dst:?}"))?;

		// optimal batch size based on number of CPUs
		let batch_size = std::cmp::max(4, num_cpus::get());
		let mut any_copied = false;

		// processing directory entries in a streaming fashion
		let entries = fs::read_dir(src).with_context(|| format!("Failed to read source directory: {src:?}"))?;

		let mut batch = Vec::with_capacity(batch_size);

		for entry_result in entries {
			let entry = entry_result?;
			let src_path = entry.path();
			let dst_path = dst.join(entry.file_name());
			let file_type = entry.file_type().with_context(|| format!("Failed to get file type for: {src_path:?}"))?;

			// a future for this file/directory operation
			let future = async move {
				if file_type.is_dir() {
					Self::copy_dir_all(&src_path, &dst_path).await
				} else if file_type.is_file() {
					Self::copy_file(&src_path, &dst_path).await
				} else {
					debug!("Skipping non-regular file: {:?}", src_path);
					Ok(false)
				}
			};

			batch.push(future);

			// batch is full, process it
			if batch.len() >= batch_size {
				let results = try_join_all(batch).await?;
				any_copied |= results.into_iter().any(|copied| copied);
				batch = Vec::with_capacity(batch_size);
			}
		}

		// process any remaining items
		if !batch.is_empty() {
			let results = try_join_all(batch).await?;
			any_copied |= results.into_iter().any(|copied| copied);
		}

		Ok(any_copied)
	}

	pub async fn copy_file_to_dist(self, config: &ExtConfig) -> Result<()> {
		info!("Copying {:?}...", self);
		let src = self.get_copy_src(config);
		let dest = self.get_copy_dest(config);

		let result = if src.is_dir() { Self::copy_dir_all(&src, &dest).await } else { Self::copy_file(&src, &dest).await };

		match result {
			Ok(copied) => {
				if copied {
					info!("[SUCCESS] Copied {:?}", self);
				} else {
					info!("[SKIPPED] No changes for {:?}", self);
				}
				Ok(())
			},
			Err(e) => {
				warn!("Copy for {:?} failed: {}", self, e);
				Err(e)
			},
		}
	}

	// the file path string for file watching
	pub fn get_watch_path(&self, config: &ExtConfig) -> String {
		match self {
			Self::Manifest => "manifest.json".to_owned(),
			Self::IndexHtml => "index.html".to_owned(),
			Self::IndexJs => "index.js".to_owned(),
			Self::BackgroundScript => config.background_script_index_name.clone(),
			Self::ContentScript => config.content_script_index_name.clone(),
			Self::Assets => config.assets_dir.clone(),
		}
	}
}
