use crate::common::{ExtConfig, FILE_HASHES, FILE_TIMESTAMPS};
use anyhow::{Context, Result};
use async_walkdir::{DirEntry, Filtering, WalkDir};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter, strum::Display)]
pub(crate) enum EFile {
	// fixed files for Chrome extensions
	Manifest,
	IndexHtml,
	IndexJs,
	// dynamic files from config
	OptionsHtml,
	OptionsJs,
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
			Self::OptionsHtml => base_path.join("options.html"),
			Self::OptionsJs => base_path.join("options.js"),
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
			Self::OptionsHtml => dist_path.join("options.html"),
			Self::OptionsJs => dist_path.join("options.js"),
			Self::BackgroundScript => dist_path.join(&config.background_script_index_name),
			Self::ContentScript => dist_path.join(&config.content_script_index_name),
			Self::Assets => dist_path.join("assets"),
		}
	}

	pub async fn copy_file_to_dist(self, config: &ExtConfig) -> Result<()> {
		info!("Copying {:?}...", self);
		let src = self.get_copy_src(config);
		let dest = self.get_copy_dest(config);
		let result = if src.is_dir() { copy_dir_all(&src, &dest).await } else { copy_file(&src, &dest).await };
		match result {
			Ok(copied) => {
				if copied != 0 {
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
			Self::OptionsHtml => "options.html".to_owned(),
			Self::OptionsJs => "options.js".to_owned(),
			Self::BackgroundScript => config.background_script_index_name.clone(),
			Self::ContentScript => config.content_script_index_name.clone(),
			Self::Assets => config.assets_dir.clone(),
		}
	}
}

// directory copy with parallel processing and hash checking
async fn copy_dir_all(src: &Path, dst: &Path) -> Result<usize> {
	let src_owned = src.to_owned();
	let dst_owned = dst.to_owned();
	Ok(
		WalkDir::new(src)
			.filter(move |entry| {
				let src = src_owned.clone();
				let dst = dst_owned.clone();
				async move { file_filter(entry, src, dst).await }
			})
			.filter_map(|entry| async move { entry.ok() })
			.then(async |entry| {
				let src_path = entry.path();
				let rel_path = src_path.strip_prefix(src).context("Failed to get relative path")?;
				let dst_path = dst.join(rel_path);
				copy_file(&src_path, &dst_path).await
			})
			.collect::<Vec<_>>()
			.await
			.into_iter()
			.filter_map(|t| t.ok())
			.sum(),
	)
}

async fn file_filter(entry: DirEntry, src: PathBuf, dst: PathBuf) -> Filtering {
	match entry.file_type().await {
		Ok(ft) if ft.is_file() => {
			let src_path = entry.path();
			let Ok(rel_path) = src_path.strip_prefix(src).context("Failed to get relative path") else {
				return Filtering::Ignore;
			};
			let dst_path = dst.join(rel_path);
			match needs_copy(&src_path, &dst_path).await {
				Ok(should_copy) => {
					if should_copy {
						Filtering::Continue
					} else {
						Filtering::Ignore
					}
				},
				Err(_) => Filtering::Ignore,
			}
		},
		_ => Filtering::Ignore,
	}
}

async fn calculate_file_hash(path: &Path) -> Result<String> {
	let data = tokio::fs::read(path).await.with_context(|| format!("Failed to read file: {path:?}"))?;
	tokio::task::spawn_blocking(move || blake3::hash(&data).to_hex().to_string()).await.context("Hash calculation task failed")
}

async fn needs_copy(src: &Path, dest: &Path) -> Result<bool> {
	let src_metadata = tokio::fs::metadata(src).await.with_context(|| format!("Failed to get metadata for source file: {src:?}"))?;
	let src_len = src_metadata.len();
	let src_time = src_metadata.modified().ok();
	if !tokio::fs::try_exists(dest).await.unwrap_or(false) {
		return Ok(true);
	}
	let dest_metadata = tokio::fs::metadata(dest).await.with_context(|| format!("Failed to get metadata for destination file: {dest:?}"))?;
	// if sizes differ, definitely needs copy
	if src_len != dest_metadata.len() {
		return Ok(true);
	}
	// timestamps checks
	if let Some(src_time) = src_time
		&& let Some(stored_time) = FILE_TIMESTAMPS.get(src)
		&& *stored_time == src_time
	{
		// file hasn't changed since last check
		return Ok(false);
	}
	// hashes comparison for final determination
	let src_hash = calculate_file_hash(src).await?;
	let dest_hash = calculate_file_hash(dest).await?;
	FILE_HASHES.insert(src.to_path_buf(), src_hash.clone());
	if let Some(src_time) = src_time {
		FILE_TIMESTAMPS.insert(src.to_path_buf(), src_time);
	}

	Ok(src_hash != dest_hash)
}

// hash checking to avoid unnecessary copies
async fn copy_file(src: &Path, dest: &Path) -> Result<usize> {
	if !tokio::fs::try_exists(src).await.unwrap_or(false) {
		return Err(anyhow::anyhow!("Source file does not exist: {src:?}"));
	}
	if let Some(parent) = dest.parent() {
		tokio::fs::create_dir_all(parent).await.with_context(|| format!("Failed to create parent directory: {parent:?}"))?;
	}
	tokio::fs::copy(src, dest).await.with_context(|| format!("Failed to copy file from {src:?} to {dest:?}"))?;
	debug!("Copied file: {:?} -> {:?}", src, dest);
	Ok(1)
}
