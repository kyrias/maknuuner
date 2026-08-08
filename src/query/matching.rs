//! Query match implementations.
//!
//! This module contains implementations of the `Matches` trait on the `Query` and `Leaf` types
//! which together form the logic for checking whether a search query matches a given lemma.

use crate::{
    lexicon::{Definition, Lemma, Phrase},
    query::{Leaf, Operator, Qualifier, Query, Term},
    string::{
        CaseFoldedNfkcNormalizedString, NfkcNormalizedString, NonFoldedNfkcNormalizedString,
        SearchableString,
    },
};

pub(crate) trait Matches<T>
where
    T: ?Sized,
{
    fn matches(&self, value: &T) -> bool;
}

impl Matches<Lemma> for Query {
    fn matches(&self, value: &Lemma) -> bool {
        match self {
            Query::Leaf(leaf) => {
                leaf.matches(value)
                    || value.definitions.iter().any(|def| leaf.matches(def))
                    || value.phrases.iter().any(|ph| leaf.matches(ph))
            }

            Query::Operator { op, lhs, rhs } => {
                let lhs = lhs.matches(value);
                match op {
                    Operator::And => lhs && rhs.matches(value),
                    Operator::Or => lhs || rhs.matches(value),
                }
            }
        }
    }
}

impl Matches<Lemma> for Leaf {
    fn matches(&self, value: &Lemma) -> bool {
        match self {
            Leaf::Term { term } => {
                term.matches(&value.root)
                    || term.matches(&value.lemma)
                    || term.matches(&value.lemma_search)
            }

            Leaf::Qualified { qualifier, term } => match qualifier {
                Qualifier::Lemma => {
                    term.matches(&value.lemma)
                        || term.matches(&value.lemma_search)
                        || term.matches(&value.lemma_bw)
                }
                Qualifier::Root => term.term.non_folded.as_str() == value.root.searchable.as_str(),
                Qualifier::Analysis | Qualifier::Gloss => false,
            },
        }
    }
}

impl Matches<Definition> for Leaf {
    fn matches(&self, value: &Definition) -> bool {
        match self {
            Leaf::Term { term } => {
                term.matches(&value.form)
                    || term.matches(&value.transcription.bw)
                    || value
                        .glosses_english
                        .iter()
                        .any(|gloss| term.matches(gloss))
            }
            Leaf::Qualified { qualifier, term } => match qualifier {
                Qualifier::Lemma => {
                    term.matches(&value.form)
                        || term.matches(&value.transcription.bw)
                        || term.matches(&value.transcription.caphipp)
                }
                Qualifier::Analysis => term.matches(&value.analysis),
                Qualifier::Gloss => value
                    .glosses_english
                    .iter()
                    .any(|gloss| term.matches(gloss)),
                Qualifier::Root => false,
            },
        }
    }
}

impl Matches<Phrase> for Leaf {
    fn matches(&self, value: &Phrase) -> bool {
        match self {
            Leaf::Term { term } => {
                term.matches(&value.form)
                    || term.matches(&value.transcription.bw)
                    || value
                        .glosses_english
                        .iter()
                        .any(|gloss| term.matches(gloss))
            }
            Leaf::Qualified { qualifier, term } => match qualifier {
                Qualifier::Lemma => {
                    term.matches(&value.form)
                        || term.matches(&value.transcription.bw)
                        || term.matches(&value.transcription.caphipp)
                }
                Qualifier::Analysis => false,
                Qualifier::Gloss => value
                    .glosses_english
                    .iter()
                    .any(|gloss| term.matches(gloss)),
                Qualifier::Root => false,
            },
        }
    }
}

impl Matches<NonFoldedNfkcNormalizedString> for Term {
    fn matches(&self, string: &NonFoldedNfkcNormalizedString) -> bool {
        let string = string.as_str();
        match (self.anchor_start, self.anchor_end) {
            (false, false) => string.contains(self.term.non_folded.as_str()),
            (true, false) => string.starts_with(self.term.non_folded.as_str()),
            (true, true) => string == self.term.non_folded.as_str(),
            (false, true) => string.ends_with(self.term.non_folded.as_str()),
        }
    }
}

impl Matches<CaseFoldedNfkcNormalizedString> for Term {
    fn matches(&self, string: &CaseFoldedNfkcNormalizedString) -> bool {
        let string = string.as_str();
        match (self.anchor_start, self.anchor_end) {
            (false, false) => string.contains(self.term.case_folded.as_str()),
            (true, false) => string.starts_with(self.term.case_folded.as_str()),
            (true, true) => string == self.term.case_folded.as_str(),
            (false, true) => string.ends_with(self.term.case_folded.as_str()),
        }
    }
}

impl Matches<SearchableString> for Term {
    fn matches(&self, value: &SearchableString) -> bool {
        match &value.searchable {
            NfkcNormalizedString::NonFolded(inner) => self.matches(inner),
            NfkcNormalizedString::CaseFolded(inner) => self.matches(inner),
        }
    }
}
