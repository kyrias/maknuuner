use std::{
    collections::{HashMap, hash_map::Entry as HMEntry},
    sync::Arc,
};

use anyhow::{Context as _, Result, ensure};
use compact_str::ToCompactString;

use crate::{
    lexicon::lemma::DatasetEntry,
    query::{self, Matches},
    string::{NfkcNormalizedString, NonFoldedNfkcNormalizedString},
    tf_idf::{DocumentTermFrequencies, InverseDocumentFrequencies, Rank, ToTerms},
};

pub(crate) use lemma::{Definition, Lemma, Phrase, Root, Transcription};

mod lemma;
mod phonetics;
pub(crate) mod pos;

pub(crate) struct Lexicon {
    pub lemmas: Arc<Vec<Lemma>>,
    pub inverse_doc_freqs: Arc<InverseDocumentFrequencies>,
    pub term_freqs: Arc<HashMap<u32, DocumentTermFrequencies>>,
}

impl Lexicon {
    pub(crate) fn new() -> Result<Self> {
        const LEXICON: &str = include_str!("../../maknuune-v1.0.1.tsv");

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b'\t')
            .from_reader(LEXICON.as_bytes());

        // Group the raw dataset entries by root, lemma, and part of speech.
        let lemmas = {
            let mut lemmas =
                HashMap::<(Root, NfkcNormalizedString, NonFoldedNfkcNormalizedString), Lemma>::new(
                );
            for entry in reader.deserialize::<DatasetEntry>() {
                let entry = entry.context("Failed to deserialize lexicon entry")?;
                let Some(entry) = patch_entry(entry).context("Failed to patch entry")? else {
                    continue;
                };

                let pos = NonFoldedNfkcNormalizedString::interned(
                    entry.analysis.split(':').next().unwrap().chars(),
                );

                let lemma =
                    Lemma::try_from(entry).context("Failed to convert DatasetEntry to lemma")?;
                let new_id = lemma.lowest_id;

                match lemmas.entry((lemma.root.clone(), lemma.lemma_bw.searchable.clone(), pos)) {
                    HMEntry::Occupied(occupied) => {
                        let existing = occupied.into_mut();
                        let existing_id = existing.lowest_id;
                        existing.merge(lemma).with_context(|| {
                            format!("Failed to merge new lemma ({new_id}) into existing lemma ({existing_id})",)
                        })?;
                    }
                    HMEntry::Vacant(vacant) => {
                        vacant.insert_entry(lemma);
                    }
                };
            }

            // Sort definitions and phrases by part of speech and ID.
            for lemma in lemmas.values_mut() {
                lemma.definitions.sort_by_key(|entry| (entry.pos, entry.id));
                lemma.phrases.sort_by_key(|entry| entry.id);
            }

            // Sort lemmas by lowest ID among entries and phrases to have a consistent order.
            //
            // This is only necessary since we don't perform the ranking step of a proper search
            // engine.
            let mut lemmas = lemmas.into_values().collect::<Vec<_>>();
            lemmas.sort_by_key(|lemma| lemma.lowest_id);

            lemmas
        };

        let mut idf = InverseDocumentFrequencies::new();
        let mut term_freqs = HashMap::new();
        for lemma in &lemmas {
            let tfs = DocumentTermFrequencies::from(lemma);
            idf.add_document(&tfs);
            term_freqs.insert(lemma.lowest_id, tfs);
        }

        Ok(Self {
            lemmas: Arc::new(lemmas),
            inverse_doc_freqs: Arc::new(idf),
            term_freqs: Arc::new(term_freqs),
        })
    }

    pub(crate) fn search(&self, query: &query::Query) -> impl Iterator<Item = (&Lemma, f64)> {
        self.lemmas
            .iter()
            .filter(|lemma| query.matches(lemma))
            .map(|lemma| (lemma, query.rank(self, lemma)))
    }

    /// Calculate the TF-IDF score of a query term against a lemma.
    pub(crate) fn tf_idf(&self, lemma: &Lemma, term: &query::Term) -> f64 {
        let term_freqs = self.term_freqs.get(&lemma.lowest_id).unwrap();

        term.to_terms()
            .map(|term| {
                let tf = term_freqs.term_frequency(&term);
                let idf = self.inverse_doc_freqs.idf(&term);

                tf * idf
            })
            .sum()
    }
}

/// Apply patches to dataset entries with issues.
fn patch_entry(mut entry: DatasetEntry) -> Result<Option<DatasetEntry>> {
    match entry.id {
        2737 => {
            ensure!(entry.root_ntws.unwrap() == "ب.ي.ر");
            entry.root_ntws = Some("ب.ي.ر.و".to_compact_string());
        }

        // Duplicates of 29603, 29605, 29612 but with different orders of the fatah and shadda in the
        // lemma.
        29598 | 29607 | 29614 => {
            ensure!(entry.lemma_bw.unwrap() == "laq~aT");
            return Ok(None);
        }

        // Invalid CAPHI++, II instead of || for alternate phonemes.
        15942 | 16642 | 16643 | 16644 | 16658 | 16659 | 16660 | 16661 | 17007 | 17008 | 25633
        | 25634 | 25716 | 26050 | 26054 | 26264 => {
            ensure!(entry.caphipp.contains("II"));
            entry.caphipp = entry.caphipp.replace("II", "||").into();
        }
        // Same but in the gloss field.
        18000 => {
            ensure!(entry.gloss.contains("II"));
            entry.gloss = entry.gloss.replace("II", "||").into();
        }
        // Same but in the notes field.
        25861 => {
            ensure!(entry.notes.contains("II"));
            entry.notes = entry.notes.replace("II", "||").into();
        }

        // Apparent dupe of 31411 with "wealth (type)" instead of "wealth" as the gloss.
        31411 => {
            ensure!(entry.lemma_bw.unwrap() == "maAl");
            return Ok(None);
        }

        _ => {}
    }

    match entry.analysis.as_str() {
        "NOUN:PL" => {
            entry.analysis.pop();
        }
        "NOUN:SF" => {
            entry.analysis = "NOUN:FS".to_compact_string();
        }
        _ => {}
    }

    Ok(Some(entry))
}
