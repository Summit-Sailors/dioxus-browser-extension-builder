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

		let mut buffer = [0; 32768];
		loop {
			let bytes_read = reader.read(&mut buffer).with_context(|| format!("Failed to read file for hashing: {path:?}"))?;

			if bytes_read == 0 {
				break;
			}

			hasher.update(&buffer[..bytes_read]);
		}

		Ok(hasher.finalize().to_hex().to_string())
	}

	// hash checking to avoid unnecessary copies
	async fn copy_file(src: &Path, dest: &Path) -> Result<bool> {
		if !src.exists() {
			return Err(anyhow::anyhow!("Source file does not exist: {:?}", src));
		}

		let src_metadata = fs::metadata(src).with_context(|| format!("Failed to get metadata for source file: {src:?}"))?;
		let src_len = src_metadata.len();
		let src_time = src_metadata.modified().ok();

		let dest_exists = dest.exists();
		let dest_metadata =
			if dest_exists { Some(fs::metadata(dest).with_context(|| format!("Failed to get metadata for destination file: {dest:?}"))?) } else { None };

		let hashes = FILE_HASHES.lock().await;
		let mut timestamps = FILE_TIMESTAMPS.lock().await;

		let copy_needed = if dest_exists && dest_metadata.is_some() {
			let dest_metadata = dest_metadata.unwrap();

			if src_len != dest_metadata.len() {
				true
			} else if let Some(src_time) = src_time {
				match timestamps.get(src) {
					Some(stored_time) if *stored_time == src_time => false,
					_ => {
						drop(hashes);
						drop(timestamps);

						let src_hash = Self::calculate_file_hash(src)?;
						let dest_hash = Self::calculate_file_hash(dest)?;

						let mut hashes = FILE_HASHES.lock().await;
						let mut timestamps = FILE_TIMESTAMPS.lock().await;

						if src_hash != dest_hash {
							hashes.insert(src.to_path_buf(), src_hash);
							timestamps.insert(src.to_path_buf(), src_time);
							true
						} else {
							timestamps.insert(src.to_path_buf(), src_time);
							false
						}
					},
				}
			} else {
				drop(hashes);
				drop(timestamps);

				let src_hash = Self::calculate_file_hash(src)?;
				let dest_hash = Self::calculate_file_hash(dest)?;

				let mut hashes = FILE_HASHES.lock().await;

				hashes.insert(src.to_path_buf(), src_hash.clone());
				src_hash != dest_hash
			}
		} else {
			// store hash and timestamp for future comparisons
			if let Some(src_time) = src_time {
				timestamps.insert(src.to_path_buf(), src_time);
			}

			drop(hashes);
			drop(timestamps);

			if let Ok(hash) = Self::calculate_file_hash(src) {
				FILE_HASHES.lock().await.insert(src.to_path_buf(), hash);
			}

			true
		};

		if copy_needed {
			// create parent directories if they don't exist
			if let Some(parent) = dest.parent() {
				fs::create_dir_all(parent).with_context(|| format!("Failed to create parent directory: {parent:?}"))?;
			}

			// the actual copy
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

		let entries = fs::read_dir(src).with_context(|| format!("Failed to read source directory: {src:?}"))?.collect::<Result<Vec<_>, _>>()?;

		const BATCH_SIZE: usize = 16;
		let mut any_copied = false;

		for chunk in entries.chunks(BATCH_SIZE) {
			let futures = chunk.iter().map(|entry| {
				let src_path = entry.path();
				let dst_path = dst.join(entry.file_name());
				let file_type = entry.file_type().with_context(|| format!("Failed to get file type for: {src_path:?}"));

				async move {
					match file_type {
						Ok(ty) if ty.is_dir() => Self::copy_dir_all(&src_path, &dst_path).await,
						Ok(ty) if ty.is_file() => Self::copy_file(&src_path, &dst_path).await,
						Ok(_) => {
							debug!("Skipping non-regular file: {:?}", src_path);
							Ok(false)
						},
						Err(e) => Err(e),
					}
				}
			});

			let results = try_join_all(futures).await?;
			any_copied |= results.into_iter().any(|copied| copied);
		}

		Ok(any_copied)
	}

	pub(crate) async fn copy_file_to_dist(self, config: &ExtConfig) -> Result<()> {
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
	pub(crate) fn get_watch_path(&self, config: &ExtConfig) -> String {
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
