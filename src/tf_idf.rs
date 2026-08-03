use std::collections::HashMap;

use itertools::Itertools;

use crate::{lexicon::Lemma, string::SearchableString};

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub(crate) struct Term(pub [char; 3]);

pub(crate) trait ToTerms {
    fn to_terms(&self) -> impl Iterator<Item = Term>;
}

impl ToTerms for str {
    fn to_terms(&self) -> impl Iterator<Item = Term> {
        // Add two spaces to the start of the string to ensure that searching for short strings
        // works.
        Itertools::array_windows("  ".chars().chain(self.chars())).map(Term)
    }
}

impl ToTerms for SearchableString {
    fn to_terms(&self) -> impl Iterator<Item = Term> {
        self.normalized
            .to_terms()
            .chain(self.case_folded.to_terms())
    }
}

impl ToTerms for Lemma {
    fn to_terms(&self) -> impl Iterator<Item = Term> {
        let lemma = self
            .root
            .to_terms()
            .chain(self.lemma.to_terms())
            .chain(self.lemma_search.to_terms())
            .chain(self.lemma_bw.to_terms());

        let definitions = self.definitions.iter().flat_map(|def| {
            def.form
                .to_terms()
                .chain(def.transcription.bw.to_terms())
                .chain(def.glosses.iter().flat_map(ToTerms::to_terms))
        });

        // Not sure whether it's better to include the phrases or not.  It seems like including
        // them leads to the relative ranking being skewed very heavily, which makes sense since
        // the phrases would in general have much more text than a definition.
        //
        // Wonder if there's an easy way to include them but de-prioritized?
        //
        // let phrases = self
        //     .phrases
        //     .iter()
        //     .map(|ph| {
        //         ph.form
        //             .to_terms()
        //             .chain(ph.form_bw.to_terms())
        //             .chain(ph.glosses.iter().map(ToTerms::to_terms).flatten())
        //     })
        //     .flatten();

        lemma.chain(definitions)
    }
}

#[derive(Debug)]
pub(crate) struct DocumentTermFrequencies {
    term_count: u16,
    term_freqs: HashMap<Term, u16>,
}

impl DocumentTermFrequencies {
    pub(crate) fn term_frequency(&self, term: &Term) -> f64 {
        (self.term_freqs.get(term).copied().unwrap_or_default() + 1) as f64
            / (self.term_count + 1) as f64
    }
}

impl From<&Lemma> for DocumentTermFrequencies {
    fn from(lemma: &Lemma) -> Self {
        let mut term_count = 0;
        let mut term_freqs = HashMap::new();
        for term in lemma.to_terms() {
            term_count += 1;
            *term_freqs.entry(term).or_default() += 1;
        }

        Self {
            term_count,
            term_freqs,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InverseDocumentFrequencies {
    doc_count: u32,
    docs_with_term: HashMap<Term, u16>,
}

impl InverseDocumentFrequencies {
    pub fn new() -> Self {
        Self {
            doc_count: 0,
            docs_with_term: Default::default(),
        }
    }

    pub fn add_document(&mut self, document: &DocumentTermFrequencies) {
        self.doc_count += 1;
        for term in document.term_freqs.keys() {
            *self.docs_with_term.entry(term.clone()).or_default() += 1;
        }
    }

    pub fn idf(&self, term: &Term) -> f64 {
        let res = (self.doc_count + 1) as f64
            / (self.docs_with_term.get(term).copied().unwrap_or_default() + 1) as f64;

        res.log10()
    }
}
