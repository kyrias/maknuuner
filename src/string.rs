use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::Deref,
};

use caseless::Caseless;
use compact_str::CompactString;
use interner::global::{GlobalPool, GlobalString};
use unicode_normalization::UnicodeNormalization;

static INTERNER_POOL: GlobalPool<String> = GlobalPool::new();

/// NKFC normalized compact string.
///
/// This string type is not interned, which means that it's suitable for user input.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NormalizedCompactString(CompactString);

impl<T: AsRef<str>> From<T> for NormalizedCompactString {
    fn from(string: T) -> Self {
        let normalized = string.as_ref().chars().nfkc().collect::<CompactString>();
        Self(normalized)
    }
}

impl Deref for NormalizedCompactString {
    type Target = CompactString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Interned NKFC normalized string.
///
/// This type holds an interned string, which means that it should never be used for user input
/// since that would let users fill up our interning table.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NormalizedInternedString(GlobalString);

impl NormalizedInternedString {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<T: AsRef<str>> From<T> for NormalizedInternedString {
    fn from(string: T) -> Self {
        let normalized = string.as_ref().chars().nfkc().collect::<CompactString>();
        Self(INTERNER_POOL.get(normalized))
    }
}

impl Display for NormalizedInternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl topcoat::view::NodeViewParts for &NormalizedInternedString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.as_str().into_view_parts(cx, parts);
    }
}

/// Case-folded and NKFC normalized string.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CaseFoldedInternedString(GlobalString);

impl CaseFoldedInternedString {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<T: AsRef<str>> From<T> for CaseFoldedInternedString {
    fn from(string: T) -> Self {
        let normalized = string
            .as_ref()
            .chars()
            .nfkc()
            .default_case_fold()
            .nfkc()
            .collect::<CompactString>();
        Self(INTERNER_POOL.get(normalized))
    }
}

/// Searchable string type.
///
/// Contains a case-folded version of the string in addition to the regular one.
///
/// All operations on this string other than displaying it to the user should use the `normalized`
/// field.
#[derive(Clone, Eq)]
pub(crate) struct SearchableInternedString {
    pub display: NormalizedInternedString,
    pub folded: CaseFoldedInternedString,
}

impl<T: AsRef<str>> From<T> for SearchableInternedString {
    fn from(value: T) -> Self {
        Self {
            display: value.as_ref().into(),
            folded: value.as_ref().into(),
        }
    }
}

impl PartialEq for SearchableInternedString {
    fn eq(&self, other: &Self) -> bool {
        self.folded == other.folded
    }
}

impl Hash for SearchableInternedString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.folded.hash(state);
    }
}

impl Debug for SearchableInternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display.as_str())
    }
}

impl topcoat::view::NodeViewParts for &SearchableInternedString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.display.into_view_parts(cx, parts);
    }
}
