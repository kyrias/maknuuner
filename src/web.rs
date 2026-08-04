use anyhow::Context as _;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt as _, asset},
    font::{Font, font},
    router::{Router, RouterBuilderDiscoverExt as _, error::redirect, layout, page},
    view::view,
};

use crate::lexicon::Lexicon;

mod search;

const AMIRI: Font = font! {
    "Amiri",
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-Regular.ttf")) format("truetype");
        font-style: normal;
        font-weight: normal;
        font-display: swap;
    }
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-Italic.ttf")) format("truetype");
        font-style: italic;
        font-weight: normal;
        font-display: swap;
    }
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-Bold.ttf")) format("truetype");
        font-style: normal;
        font-weight: bold;
        font-display: swap;
    }
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-BoldItalic.ttf")) format("truetype");
        font-style: italic;
        font-weight: bold;
        font-display: swap;
    }
};

pub(super) async fn start(lexicon: Lexicon) -> anyhow::Result<()> {
    topcoat::start(
        Router::builder()
            .discover()
            .app_context(lexicon)
            .assets(AssetBundle::load().context("Failed to load asset bundle")?)
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
                topcoat::dev::script()
                topcoat::runtime::script()
                topcoat::font::link(font: AMIRI)
                <style>
                    "
*, *::before, *::after { box-sizing: border-box; }
* { margin: 0; padding: 0; }
input, button, textarea, select {
  font: inherit;
}

:root {
    font-family: "
                    (AMIRI.family())
                    ", serif;
    font-size: 14pt;
    line-height: 1.85;
    text-wrap: balance;
}

:lang(ar) {
    font-size: 140%;
}

:lang(en) {
}

html {
    padding-left: 0.5em;
    padding-right: 0.5em;
}

input {
    padding-left: 0.3em;
    padding-right: 0.3em;
}

ol {
    padding-left: 2em;
}
"
                    (topcoat::view::Unescaped::new_unchecked(
                        "

input.search {
    margin-top: 0.5em;
    width: 20em;
}

.results {
    margin-top: 1em;

    & .inner {
        padding-left: 0.75em;
    }
}

.result {
    &:not(:first-child) {
        margin-top: 0.75em;
    }

    h3 {
        & .root {
            font-size: 0.9em;
        }
        & .lemma {
            margin-left: 0.5em;
        }

        & a {
            margin-left: 0.75em;

            font-size: 1em;
            color: oklch(0.5999 0 0 / 40%);
            transition: color 0.2s;

            text-decoration: none;
        }
        &:hover a {
                color: oklch(0.469 0.224 321.186);
        };
    }

    .pos {
        margin-left: 0.75em;
    }
    .transcription {
        margin-left: 0.75em;
        color: oklch(0.5 0 0);
    }
}
",
                    ))
                </style>
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
