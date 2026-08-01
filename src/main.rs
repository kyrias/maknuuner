use std::{fmt::Debug, ops::Deref};

use anyhow::{Context, Result};
use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

mod lexicon;
mod query;
mod web;

/// Case-folded and NKFC normalized string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NormalizedString(String);

impl Deref for NormalizedString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for NormalizedString {
    fn from(raw: &str) -> Self {
        let normalized = raw
            .chars()
            .nfkc()
            .default_case_fold()
            .nfkc()
            .collect::<String>();
        Self(normalized)
    }
}

/// String type containing the original string for display purposes in addition to a case-folded
/// and NKFC normalized string for searching.
#[derive(Clone)]
pub(crate) struct Str {
    pub raw: String,
    pub normalized: NormalizedString,
}

impl From<String> for Str {
    fn from(raw: String) -> Self {
        let normalized = raw.as_str().into();

        Self { raw, normalized }
    }
}

impl PartialEq for Str {
    fn eq(&self, other: &Self) -> bool {
        self.normalized == other.normalized
    }
}

impl Debug for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.raw)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let lexicon = lexicon::Lexicon::new().context("Failed to parse lexicon")?;

    web::start(lexicon)
        .await
        .context("Failed to start web server")?;

    Ok(())
}
