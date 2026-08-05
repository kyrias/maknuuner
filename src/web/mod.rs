use anyhow::Context as _;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt as _},
    router::{Router, RouterBuilderDiscoverExt as _, error::redirect, layout, page},
    view::view,
};

use crate::lexicon::Lexicon;

mod search;
mod styles;
mod title;

pub(super) async fn start(asset_bundle: AssetBundle, lexicon: Lexicon) -> anyhow::Result<()> {
    topcoat::start(
        Router::builder()
            .discover()
            .app_context(lexicon)
            .assets(asset_bundle)
            .build(),
    )
    .await
    .context("Failed to start topcoat")
}

#[layout("/")]
async fn root_layout(slot: Result) -> Result {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                title::title()

                topcoat::dev::script()
                topcoat::runtime::script()

                styles::styles()
            </head>
            <body>
                <header><h1>"Maknuuner"</h1></header>
                (slot?)
            </body>
        </html>
    }
}

#[page("/")]
async fn home() -> Result {
    Err(redirect("/search").into())
}
