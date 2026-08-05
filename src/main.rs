use anyhow::Context;
use topcoat::asset::AssetBundle;

mod lexicon;
mod query;
mod string;
mod tf_idf;
mod web;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let asset_bundle = AssetBundle::load().context("Failed to load asset bundle")?;

    println!("Creating lexicon");
    let lexicon = lexicon::Lexicon::new(&asset_bundle).context("Failed to parse lexicon")?;
    println!("Lexicon created");

    web::start(asset_bundle, lexicon)
        .await
        .context("Failed to start web server")?;

    Ok(())
}
