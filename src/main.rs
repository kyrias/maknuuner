use anyhow::Context;

mod lexicon;
mod query;
mod string;
mod tf_idf;
mod web;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Creating lexicon");
    let lexicon = lexicon::Lexicon::new().context("Failed to parse lexicon")?;
    println!("Lexicon created");

    web::start(lexicon)
        .await
        .context("Failed to start web server")?;

    Ok(())
}
