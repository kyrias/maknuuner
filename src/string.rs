use std::{fmt::Debug, hash::Hash, ops::Deref};

use caseless::Caseless;
use compact_str::CompactString;
use interner::global::{GlobalPool, GlobalString};
use unicode_normalization::UnicodeNormalization;

static INTERNER_POOL: GlobalPool<String> = GlobalPool::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InternedString(GlobalString);

/// Interned string type.
///
/// This type performs no normalization or case-folding.  Outside of the `string` module it should
/// **only** be used by the `Transcription` struct which needs it as a result of NKFC normalization
/// screwing up IPA modifier characters.
impl InternedString {
    pub(crate) fn new<T: AsRef<str>>(string: T) -> Self {
        Self(INTERNER_POOL.get(string.as_ref()))
    }
}

impl Deref for InternedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// NKFC normalized string type.
///
/// Every string in the application should use either this string type or one built on top of it to
/// ensure that we normalize every string passing through it.
#[derive(Clone, Debug, Eq)]
pub(crate) enum NormalizedString {
    Interned(InternedString),
    Allocated(CompactString),
}

impl NormalizedString {
    fn normalize<T: IntoIterator<Item = char>>(string: T) -> CompactString {
        string.into_iter().nfkc().collect::<CompactString>()
    }

    pub(crate) fn interned<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self::Interned(InternedString::new(NormalizedString::normalize(string)))
    }

    pub(crate) fn allocated<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self::Allocated(NormalizedString::normalize(string))
    }
}

impl Deref for NormalizedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            NormalizedString::Interned(inner) => inner,
            NormalizedString::Allocated(inner) => inner,
        }
    }
}

impl Hash for NormalizedString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let s: &str = self;
        s.hash(state);
    }
}

impl PartialEq for NormalizedString {
    fn eq(&self, other: &Self) -> bool {
        // Interned strings can be cheaply compared against each other, all other strings must be
        // compared as &str.
        match (self, other) {
            (Self::Interned(this), Self::Interned(other)) => this == other,
            _ => {
                let this: &str = self;
                let other: &str = other;

                this == other
            }
        }
    }
}

impl PartialOrd for NormalizedString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NormalizedString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let this: &str = self;
        let other: &str = other;

        this.cmp(other)
    }
}

impl topcoat::view::NodeViewParts for &NormalizedString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        let s: &str = self;
        s.into_view_parts(cx, parts);
    }
}

/// Case-folded string type.
///
/// This is useful for fields for which case-insensitive search makes sense but which we don't need
/// to display to the user in its original form.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CaseFoldedString(NormalizedString);

impl CaseFoldedString {
    fn case_fold<T: IntoIterator<Item = char>>(string: T) -> impl Iterator<Item = char> {
        string.into_iter().nfkc().default_case_fold()
    }

    fn interned<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(NormalizedString::interned(Self::case_fold(string)))
    }

    pub(crate) fn allocated<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(NormalizedString::allocated(Self::case_fold(string)))
    }
}

impl Deref for CaseFoldedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Searchable string type.
///
/// Contains both a display version of the string as well as a case-folded version for searching.
///
/// All operations on this string other than displaying it to the user should use the `case_folded`
/// field.
#[derive(Clone, Eq)]
pub(crate) struct SearchableString {
    pub normalized: NormalizedString,
    pub case_folded: CaseFoldedString,
}

impl SearchableString {
    pub(crate) fn interned<T: IntoIterator<Item = char> + Clone>(string: T) -> Self {
        Self {
            normalized: NormalizedString::interned(string.clone()),
            case_folded: CaseFoldedString::interned(string),
        }
    }

    pub(crate) fn allocated<T: IntoIterator<Item = char> + Clone>(string: T) -> Self {
        Self {
            normalized: NormalizedString::allocated(string.clone()),
            case_folded: CaseFoldedString::allocated(string),
        }
    }
}

impl PartialEq for SearchableString {
    fn eq(&self, other: &Self) -> bool {
        self.case_folded == other.case_folded
    }
}

impl Hash for SearchableString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.case_folded.hash(state);
    }
}

impl Debug for SearchableString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let normalized: &str = &self.normalized;
        let case_folded: &str = &self.case_folded;
        f.debug_struct("SearchableString")
            .field("normalized", &normalized)
            .field("case_folded", &case_folded)
            .finish()
    }
}

impl topcoat::view::NodeViewParts for &SearchableString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.normalized.into_view_parts(cx, parts);
    }
}
