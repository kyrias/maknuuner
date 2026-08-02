use std::{fmt::Debug, hash::Hash, ops::Deref};

use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

/// Case-folded and NKFC normalized string.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Clone, Eq)]
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

impl Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.normalized.hash(state);
    }
}

impl Debug for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.raw)
    }
}
