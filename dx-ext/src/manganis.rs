use {
	crate::{BuildMode, EFile, ExtConfig},
	anyhow::{Context, Result},
	manganis_cli_support::{AssetManifestExt, ManganisSupportGuard},
	manganis_common::{AssetManifest, Config as ManganisCfg},
	std::{fs, path::Path},
	tracing::{info, warn},
};

pub(crate) struct ManganisBuildManager {
	config: ExtConfig,
	guard: Option<ManganisSupportGuard>,
}

impl ManganisBuildManager {
	pub fn new(config: ExtConfig) -> Self {
		Self { config, guard: None }
	}

	pub fn configure(&mut self) -> Result<()> {
		if self.guard.is_none() {
			let assets_serve_location = &self.config.assets_dir;

			// configure manganis
			ManganisCfg::default().with_assets_serve_location(assets_serve_location).save();

			// tell manganis that we support assets
			self.guard = Some(ManganisSupportGuard::default());
		}

		Ok(())
	}

	pub async fn process_assets(&mut self) -> Result<()> {
		if self.guard.is_none() {
			return Err(anyhow::anyhow!("Manganis Guard not initialized"));
		}

		let is_release = matches!(self.config.build_mode, BuildMode::Release);
		let args = if is_release { vec!["--release"] } else { vec![] };

		// first, check if we're in linker intercept mode
		if let Some((_working_dir, object_files)) = manganis_cli_support::linker_intercept(std::env::args()) {
			info!("Manganis linker intercept active... collecting assets");
			let manifest = AssetManifest::load(object_files);

			let ext_dir = &self.config.extension_directory_name;
			let asset_files_location = format!("./{}/dist/assets", ext_dir);

			// create assets dir if it doesn't exist
			fs::create_dir_all(&asset_files_location).with_context(|| format!("Failed to create assets directory: {}", asset_files_location))?;

			// copy static files
			manifest.copy_static_assets_to(&asset_files_location).with_context(|| "Failed to copy static assets")?;

			// process tailwindcss
			let tailwind_path = format!("{}/tailwind.css", asset_files_location);
			if self.config.assets_include_tailwind {
				let css = manifest.collect_tailwind_css(is_release, &mut Vec::new());
				fs::write(&tailwind_path, css).with_context(|| "Failed to write to tailwind.css")?;
			} else {
				let source_path = Path::new(&self.config.assets_dir).join("tailwind.css");
				if source_path.exists() {
					EFile::copy_file(&source_path, Path::new(&tailwind_path)).await.with_context(|| "Failed to copy pre-existing tailwind.css")?;
				} else {
					warn!("tailwind.css not found in assets_dir, skipping copy.");
				}
			}

			info!("[SUCCESS] Manganis asset processing done");
			Ok(())
		} else {
			// if not in intercept mode, initiate assets processing
			let ext_dir = &self.config.extension_directory_name;

			info!("Initializing Manganis assets processing...");
			manganis_cli_support::start_linker_intercept(Some(format!("./{}", ext_dir)), args).with_context(|| "Failed to start Manganis linker intercept")?;

			info!("[SUCCESS] Manganis asset processing initialized");
			Ok(())
		}
	}
}
