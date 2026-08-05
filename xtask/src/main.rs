use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use topcoat_asset::{Bundler, BundlerConfig};
use tracing::info;
use zip::ZipArchive;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let task = env::args().nth(1);
    match task.as_deref() {
        Some("assets") => get_assets(),
        Some("build") => build(),
        Some("dist") => dist(),
        Some(xtask) => bail!("Unknown xtask {xtask}.  Expected one of assets, build, dist."),
        None => bail!("No xtask specified"),
    }
}

fn get_assets() -> Result<()> {
    info!("Preparing assets");

    let asset_dir = asset_dir();
    fs::create_dir_all(&asset_dir).context("Could not create asset dir")?;

    let font_archive_path = asset_dir.join("Amiri-1.003.zip");
    let fetch_font = || -> Result<()> {
        const FONT_URL: &str =
            "https://github.com/aliftype/amiri/releases/download/1.003/Amiri-1.003.zip";
        let mut res = reqwest::blocking::get(FONT_URL)
            .context("Failed to send request")?
            .error_for_status()
            .context("Fetching font returned HTTP error")?;

        let mut f =
            File::create(&font_archive_path).context("Could not create font output file")?;

        res.copy_to(&mut f)
            .context("Could not copy response data to output file")?;

        Ok(())
    };
    if font_archive_path.exists() {
        info!("Font already downloaded");
    } else {
        info!("Downloading font");
        fetch_font().context("Could not fetch font")?;
    }

    if asset_dir.join("Amiri-1.003").exists() {
        info!("Font already extracted");
    } else {
        info!("Extracting font");
        let mut zip = File::open(&font_archive_path)
            .map_err(anyhow::Error::from)
            .map(|f| ZipArchive::new(f).map_err(anyhow::Error::from))
            .flatten()
            .context("Could not open font archive")?;
        zip.extract(&asset_dir)
            .context("Could not extract font archive")?;
    }

    Ok(())
}

fn build() -> Result<()> {
    get_assets().context("Could not prepare assets")?;

    info!("Building binary");
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(&["build", "--release"])
        .status()
        .context("Could not execute cargo")?;
    if !status.success() {
        bail!("Building binary failed");
    }

    Ok(())
}

fn dist() -> Result<()> {
    build()?;

    info!("Collecting dist package");

    fs::create_dir_all(dist_dir()).context("Could not create dist directory")?;

    let bin_path = target_dir().join("release/maknuuner");
    fs::copy(&bin_path, dist_dir().join("maknuuner"))
        .context("Could not copy binary to dist directory")?;

    let binary = fs::read(&bin_path).context("Could not read binary")?;
    Bundler::new(&BundlerConfig::new().cache_dir(target_dir().join("topcoat/cache/assets")))
        .bundle(&binary, dist_dir().join("assets"))
        .context("Could not bundle assets")?;

    info!("Built dist package available in {}", dist_dir().display());

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn target_dir() -> PathBuf {
    project_root().join("target")
}

fn asset_dir() -> PathBuf {
    project_root().join("assets")
}

fn dist_dir() -> PathBuf {
    project_root().join("dist")
}
