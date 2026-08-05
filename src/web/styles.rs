use topcoat::{
    Result,
    asset::asset,
    font::{Font, font},
    view::{component, view},
};

const AMIRI: Font = font! {
    "Amiri",
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-Regular.ttf")) format("truetype");
        font-style: normal;
        font-weight: normal;
        font-display: block;
    }
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-Italic.ttf")) format("truetype");
        font-style: italic;
        font-weight: normal;
        font-display: block;
    }
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-Bold.ttf")) format("truetype");
        font-style: normal;
        font-weight: bold;
        font-display: block;
    }
    @font-face {
        src: url(asset!("assets/Amiri-1.003/Amiri-BoldItalic.ttf")) format("truetype");
        font-style: italic;
        font-weight: bold;
        font-display: block;
    }
};

#[component]
pub(super) async fn styles() -> Result {
    view! {
        topcoat::font::link(font: AMIRI)
        reset_styles()
        common_styles()
        page_styles()
    }
}

#[component]
async fn reset_styles() -> Result {
    let s = "
*, *::before, *::after {
    box-sizing: border-box;
}

* {
    margin: 0;
    padding: 0;
}

input, button, textarea, select {
  font: inherit;
}
";
    view! { <style>(s)</style> }
}

#[component]
async fn common_styles() -> Result {
    view! {
        <style>
            "
:root {
    font-family: "
            (AMIRI.family())
            ", serif;
    font-size: 14pt;
    line-height: 1.85;
    text-wrap: pretty;
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
    padding-inline-start: 1lh;
}
"
        </style>
    }
}

// TODO: It would probably make sense to handle per-page styles in the same way as `web::title`.
#[component]
async fn page_styles() -> Result {
    let s = "
input.search {
    margin-top: 0.5em;
    width: 100%;
    max-width: 20em;
}

.results {
    margin-top: 1em;

    & .inner {
        padding-left: 0.75em;
    }
}

.result {
    &:not(:first-child) {
        margin-top: 1em;
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

    h4 {
        margin-top: 0.5em;
    }

    .form-header {
        display: inline flex;
        align-items: baseline;
        column-gap: 0.75em;
        flex-wrap: wrap;
    }

    .transcription, .msa-glosses {
        & .list {
            display: inline flex;
            align-items: baseline;
            column-gap: 0.4em;
            flex-wrap: wrap;

        }

        & > :first-child {
            margin-right: 0.1em;
        }
        & > :last-child {
            margin-left: 0.1em;
        }

        color: oklch(0.5 0 0);
    }

    .examples {
        margin-top: 0.5em;

        font-size: 1em;
        color: oklch(0.5 0 0);

        .example-header {
            font-weight: bold;
            text-decoration: underline;
        }
    }
}

.raw {
    font-size: 8pt;
}
";
    view! { <style>(topcoat::view::Unescaped::new_unchecked(s))</style> }
}
