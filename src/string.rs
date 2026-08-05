use std::{fmt::Debug, hash::Hash};

use caseless::Caseless;
use compact_str::{CompactString, ToCompactString};
use interner::global::{GlobalPool, GlobalString};
use unicode_normalization::UnicodeNormalization;

static INTERNER_POOL: GlobalPool<std::string::String> = GlobalPool::new();

/// Interned or heap allocated string type.
///
/// This type performs no normalization or case-folding.
#[derive(Clone, Debug)]
enum String {
    Interned(GlobalString),
    Allocated(CompactString),
}

impl String {
    pub(crate) fn interned<T: AsRef<str>>(string: T) -> Self {
        Self::Interned(INTERNER_POOL.get(string.as_ref().trim()))
    }

    pub(crate) fn allocated<T: Into<CompactString>>(string: T) -> Self {
        Self::Allocated(string.into().trim().to_compact_string())
    }

    fn as_str(&self) -> &str {
        match self {
            String::Interned(inner) => inner,
            String::Allocated(inner) => inner,
        }
    }
}

impl Hash for String {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq for String {
    fn eq(&self, other: &Self) -> bool {
        // Interned strings can be cheaply compared against each other, all other strings must be
        // compared as &str.
        match (self, other) {
            (Self::Interned(this), Self::Interned(other)) => this == other,
            _ => self.as_str() == other.as_str(),
        }
    }
}

impl Eq for String {}

impl PartialOrd for String {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for String {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

/// NFC normalized string type.
///
/// Every string in the application that should be displayable should be built on top of this
/// string.
///
/// Note: NFC normalization is only useful for rendering, it is not appropriate for searching as
/// semantically equivalent characters aren't necessarily always encoded in the same way.
#[derive(Clone, PartialEq)]
pub(crate) struct NfcNormalizedString(String);

impl Debug for NfcNormalizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NfcNormalizedString({:?})", self.0.as_str())
    }
}

impl NfcNormalizedString {
    fn normalize<T: IntoIterator<Item = char>>(string: T) -> CompactString {
        string.into_iter().nfc().collect::<CompactString>()
    }

    pub(crate) fn interned<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(String::interned(Self::normalize(string)))
    }

    pub(crate) fn allocated<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(String::allocated(Self::normalize(string)))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl topcoat::view::NodeViewParts for &NfcNormalizedString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.as_str().into_view_parts(cx, parts);
    }
}

#[derive(Clone)]
pub(crate) enum NfkcNormalizedString {
    NonFolded(NonFoldedNfkcNormalizedString),
    CaseFolded(CaseFoldedNfkcNormalizedString),
}

impl NfkcNormalizedString {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            NfkcNormalizedString::NonFolded(inner) => inner.as_str(),
            NfkcNormalizedString::CaseFolded(inner) => inner.as_str(),
        }
    }
}

/// Non-folded NFKC normalized string type.
///
/// Every string in the application that should be searchable should be built on top of this
/// string.
///
/// This type should only be used for fields where case-folding changes the semantics, such as the
/// Buckwalter transliteration.  In all other cases [`CaseFoldedNfkcNormalizedString`] is more
/// appropriate.
///
/// Note: NFKC normalization is only useful for searching, it is not appropriate for rendering as
/// it will sometimes decompose characters into characters that render differently, especially in
/// the case of IPA transcriptions.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NonFoldedNfkcNormalizedString(String);

impl NonFoldedNfkcNormalizedString {
    fn normalize<T: IntoIterator<Item = char>>(string: T) -> CompactString {
        string.into_iter().nfkc().collect::<CompactString>()
    }

    pub(crate) fn interned<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(String::interned(Self::normalize(string)))
    }

    pub(crate) fn allocated<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(String::allocated(Self::normalize(string)))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Debug for NonFoldedNfkcNormalizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NonFoldedNfkcNormalizedString({:?})", self.0.as_str())
    }
}

/// Case-folded NFKC normalized string type.
///
/// This is useful for fields for which case-insensitive search makes sense but which we don't need
/// to display to the user in its original form.
///
/// This type should be used for all fields *except for* fields where case-folding changes the
/// semantics, such as the Buckwalter transliteration.  Those fields should use
/// [`NonFoldedNfkcNormalizedString`].
///
/// Note: NFKC normalization is only useful for searching, it is not appropriate for rendering as
/// it will sometimes decompose characters into characters that render differently, especially in
/// the case of IPA transcriptions.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CaseFoldedNfkcNormalizedString(String);

impl CaseFoldedNfkcNormalizedString {
    fn case_fold<T: IntoIterator<Item = char>>(string: T) -> CompactString {
        string
            .into_iter()
            .default_case_fold()
            .nfkc()
            .collect::<CompactString>()
    }

    pub(crate) fn interned<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(String::interned(Self::case_fold(string)))
    }

    pub(crate) fn allocated<T: IntoIterator<Item = char>>(string: T) -> Self {
        Self(String::allocated(Self::case_fold(string)))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Debug for CaseFoldedNfkcNormalizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CaseFoldedNfkcNormalizedString({:?})", self.0.as_str())
    }
}

/// Searchable string type.
///
/// Contains separate versions of the string meant for displaying or for searching.
///
/// Displayable strings *should* be NFC normalized and *must not* be NFKC normalized as NFKC
/// normalization can change how the string renders.
///
/// Searchable strings *must* be NFKC normalized and optionally case-folded.
///
/// All operations on this string other than displaying it to the user must use the `searchable`
/// field.
#[derive(Clone)]
pub(crate) struct SearchableString {
    pub(crate) displayable: NfcNormalizedString,
    pub(crate) searchable: NfkcNormalizedString,
}

impl SearchableString {
    pub(crate) fn non_folded<T: IntoIterator<Item = char> + Clone>(string: T) -> Self {
        Self {
            displayable: NfcNormalizedString::interned(string.clone()),
            searchable: NfkcNormalizedString::NonFolded(NonFoldedNfkcNormalizedString::interned(
                string,
            )),
        }
    }

    pub(crate) fn case_folded<T: IntoIterator<Item = char> + Clone>(string: T) -> Self {
        Self {
            displayable: NfcNormalizedString::interned(string.clone()),
            searchable: NfkcNormalizedString::CaseFolded(CaseFoldedNfkcNormalizedString::interned(
                string,
            )),
        }
    }
}

impl Debug for SearchableString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SearchableString({:?})", self.displayable.as_str())
    }
}

impl Hash for SearchableString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.searchable.as_str().hash(state);
    }
}

impl PartialEq for SearchableString {
    fn eq(&self, other: &Self) -> bool {
        self.searchable.as_str() == other.searchable.as_str()
    }
}

impl Eq for SearchableString {}

impl topcoat::view::NodeViewParts for &SearchableString {
    fn into_view_parts(
        self,
        cx: &topcoat::context::Cx,
        parts: &mut topcoat::view::PartsWriter<'_>,
    ) {
        self.displayable.into_view_parts(cx, parts);
    }
}
