use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::Deref,
};

use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

/// NKFC normalized string.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NormalizedString(String);

impl From<&str> for NormalizedString {
    fn from(raw: &str) -> Self {
        let normalized = raw.chars().nfkc().collect::<String>();
        Self(normalized)
    }
}

impl Deref for NormalizedString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for NormalizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl topcoat::view::NodeViewParts for &NormalizedString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.0.as_str().into_view_parts(cx, parts);
    }
}

/// Case-folded and NKFC normalized string.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CaseFoldedString(NormalizedString);

impl From<&str> for CaseFoldedString {
    fn from(raw: &str) -> Self {
        let normalized = raw
            .chars()
            .nfkc()
            .default_case_fold()
            .nfkc()
            .collect::<String>();
        Self(NormalizedString(normalized))
    }
}

impl Deref for CaseFoldedString {
    type Target = NormalizedString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Searchable string type.
///
/// Contains a case-folded version of the string in addition to the regular one.
///
/// All operations on this string other than displaying it to the user should use the `normalized`
/// field.
#[derive(Clone, Eq)]
pub(crate) struct SearchableString {
    pub display: NormalizedString,
    pub folded: CaseFoldedString,
}

impl<T: AsRef<str>> From<T> for SearchableString {
    fn from(value: T) -> Self {
        Self {
            display: value.as_ref().into(),
            folded: value.as_ref().into(),
        }
    }
}

impl PartialEq for SearchableString {
    fn eq(&self, other: &Self) -> bool {
        self.folded == other.folded
    }
}

impl Hash for SearchableString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.folded.hash(state);
    }
}

impl Debug for SearchableString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.display)
    }
}

impl topcoat::view::NodeViewParts for &SearchableString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.display.into_view_parts(cx, parts);
    }
}
