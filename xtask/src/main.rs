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

fn download_file(url: &str, dest: &Path) -> Result<()> {
    let mut res = reqwest::blocking::get(url)
        .context("Failed to send request")?
        .error_for_status()
        .context("Got HTTP error")?;

    let mut f = File::create(dest).context("Could not create output file")?;

    res.copy_to(&mut f)
        .context("Could not copy response data to output file")?;

    Ok(())
}

fn get_dataset() -> Result<()> {
    let maknuune_archive_path = asset_dir().join("maknuune-v1.zip");

    if maknuune_archive_path.exists() {
        info!("Maknuune dataset already downloaded");
    } else {
        info!("Downloading Maknuune dataset");
        const URL: &str =
            "https://drive.google.com/uc?id=1prIUi6nw9DHVkvBx0YiVcm6aQYYfqXLy&export=download";
        download_file(URL, &maknuune_archive_path).context("Could not download dataset")?;
    }

    if asset_dir().join("maknuune-v1.0.1").exists() {
        info!("Dataset already extracted");
    } else {
        info!("Extracting dataset");
        let mut zip = File::open(&maknuune_archive_path)
            .map_err(anyhow::Error::from)
            .and_then(|f| ZipArchive::new(f).map_err(anyhow::Error::from))
            .context("Could not open dataset archive")?;
        zip.extract(asset_dir())
            .context("Could not extract dataset archive")?;
    }

    Ok(())
}

fn get_font() -> Result<()> {
    let font_archive_path = asset_dir().join("Amiri-1.003.zip");

    if font_archive_path.exists() {
        info!("Font already downloaded");
    } else {
        info!("Downloading font");
        const URL: &str =
            "https://github.com/aliftype/amiri/releases/download/1.003/Amiri-1.003.zip";
        download_file(URL, &font_archive_path).context("Could not download font")?;
    }

    if asset_dir().join("Amiri-1.003").exists() {
        info!("Font already extracted");
    } else {
        info!("Extracting font");
        let mut zip = File::open(&font_archive_path)
            .map_err(anyhow::Error::from)
            .and_then(|f| ZipArchive::new(f).map_err(anyhow::Error::from))
            .context("Could not open font archive")?;
        zip.extract(asset_dir())
            .context("Could not extract font archive")?;
    }

    Ok(())
}

fn get_assets() -> Result<()> {
    info!("Preparing assets");

    let asset_dir = asset_dir();
    fs::create_dir_all(&asset_dir).context("Could not create asset dir")?;

    get_dataset()?;
    get_font()?;

    Ok(())
}

fn build() -> Result<()> {
    get_assets().context("Could not prepare assets")?;

    info!("Building binary");
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(["build", "--release"])
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
