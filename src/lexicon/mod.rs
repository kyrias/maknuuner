use std::{
    collections::{HashMap, hash_map::Entry as HMEntry},
    sync::Arc,
};

use anyhow::{Context as _, Result, bail, ensure};
use compact_str::{CompactString, ToCompactString};

use crate::{
    lexicon::pos::PartOfSpeech,
    query::{self, MatchTerm as _},
    string::{NormalizedInternedString, SearchableInternedString},
    tf_idf::{DocumentTermFrequencies, InverseDocumentFrequencies, ToTerms},
};

pub(crate) mod pos;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) struct Record {
    id: u32,

    // These are `Option` so that `Lemma::merge` can take them and still call
    // `Entry::from(Record)`.
    root: Option<CompactString>,
    root_ntws: Option<CompactString>,
    root_1: Option<CompactString>,
    lemma: Option<CompactString>,
    lemma_search: Option<CompactString>,
    lemma_bw: Option<CompactString>,

    form: CompactString,
    form_bw: CompactString,
    #[serde(rename = "CAPHI++")]
    caphipp: CompactString,
    analysis: CompactString,
    gloss: CompactString,
    gloss_msa: CompactString,
    example_usage: CompactString,
    notes: CompactString,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Root {
    Root(NormalizedInternedString),
    NonTemplaticWordStem(NormalizedInternedString),
}

impl Root {
    fn new(root: CompactString, root_ntws: CompactString) -> Self {
        if root == "NTWS" {
            Self::NonTemplaticWordStem(root_ntws.as_str().into())
        } else {
            Self::Root(root.as_str().into())
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Root::Root(s) => s.as_str(),
            Root::NonTemplaticWordStem(s) => s.as_str(),
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Custom {
    /// Parsed form of the analysis field.
    pub pos: Option<pos::PartOfSpeech>,

    /// Whether this entry was automatically generated from another one.
    ///
    /// This is designated in the TSV dataset by `[auto]` being appended to the gloss field.
    pub auto: bool,
}

// TODO: Split phrases into their own struct with only the relevant fields?  This would allow for
//       removing the phrase options from all the PoS feature types.
#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Definition {
    pub id: u32,

    pub form: NormalizedInternedString,
    pub form_bw: NormalizedInternedString,
    pub caphipp: NormalizedInternedString,
    pub analysis: SearchableInternedString,

    pub glosses: Vec<SearchableInternedString>,
    pub gloss_msa: NormalizedInternedString,

    pub example_usage: NormalizedInternedString,
    pub notes: SearchableInternedString,

    pub custom: Custom,
}

impl Definition {
    fn matches(&self, query: &query::Leaf) -> bool {
        match query {
            query::Leaf::Term { term } => {
                term.matches(&self.form)
                    || self.glosses.iter().any(|gloss| term.matches(gloss))
                    || term.matches(&self.gloss_msa)
                    || term.matches(&self.notes)
            }
            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Term => {
                    term.matches(&self.gloss_msa) || term.matches(&self.caphipp)
                }
                query::Qualifier::Analysis => term.matches(&self.analysis),
                query::Qualifier::Gloss => {
                    self.glosses.iter().any(|gloss| term.matches(gloss))
                        || term.matches(&self.gloss_msa)
                }
            },
        }
    }

    /// Parse a gloss string into a vector of glosses, removing the auto-generated entry suffix if
    /// present.
    fn parse_glosses(gloss: &str) -> (Vec<SearchableInternedString>, bool) {
        let stripped = gloss.strip_suffix("[auto]");
        let auto = stripped.is_some();

        let glosses: Vec<SearchableInternedString> = stripped
            .unwrap_or(gloss)
            .replace("_", " ")
            .split(';')
            .map(|g| g.trim().into())
            .collect();

        (glosses, auto)
    }

    fn parse_pos(id: u32, analysis: &str) -> Option<PartOfSpeech> {
        // TODO: This is a huge hack to be able to more easily incrementally start parsing the
        //       analysis field.
        if !(analysis.starts_with("NOUN") || analysis.starts_with("VERB")) {
            return None;
        }

        match analysis.parse() {
            Ok(pos) => Some(pos),
            Err(err) => {
                println!("Parsing POS failed for record {id}: {err:#?}");
                None
            }
        }
    }
}

impl TryFrom<Record> for Definition {
    type Error = anyhow::Error;

    fn try_from(record: Record) -> Result<Self, Self::Error> {
        if record.analysis.ends_with(":PHRASE") {
            bail!("Tried to convert a phrase record into Phrase");
        }

        let Record {
            id,
            root: _,
            root_ntws: _,
            root_1: _,
            lemma: _,
            lemma_search: _,
            form,
            lemma_bw: _,
            form_bw,
            caphipp,
            analysis,
            gloss,
            gloss_msa,
            example_usage,
            notes,
        } = record;

        let (glosses, auto) = Self::parse_glosses(&gloss);
        let pos = Self::parse_pos(id, &analysis);

        Ok(Self {
            id,
            form: form.as_str().into(),
            form_bw: form_bw.as_str().into(),
            caphipp: caphipp.as_str().into(),
            analysis: analysis.as_str().into(),
            glosses,
            gloss_msa: gloss_msa.as_str().into(),
            example_usage: example_usage.as_str().into(),
            notes: notes.as_str().into(),

            custom: Custom { pos, auto },
        })
    }
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Phrase {
    pub id: u32,

    pub form: NormalizedInternedString,
    pub form_bw: NormalizedInternedString,
    pub caphipp: NormalizedInternedString,

    pub glosses: Vec<SearchableInternedString>,
    pub gloss_msa: NormalizedInternedString,

    pub example_usage: NormalizedInternedString,
    pub notes: SearchableInternedString,
}

impl Phrase {
    fn matches(&self, query: &query::Leaf) -> bool {
        match query {
            query::Leaf::Term { term } => {
                term.matches(&self.form)
                    || self.glosses.iter().any(|gloss| term.matches(gloss))
                    || term.matches(&self.gloss_msa)
                    || term.matches(&self.notes)
            }
            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Term => {
                    term.matches(&self.gloss_msa) || term.matches(&self.caphipp)
                }
                query::Qualifier::Analysis => false,
                query::Qualifier::Gloss => {
                    self.glosses.iter().any(|gloss| term.matches(gloss))
                        || term.matches(&self.gloss_msa)
                }
            },
        }
    }
}

impl TryFrom<Record> for Phrase {
    type Error = anyhow::Error;

    fn try_from(record: Record) -> Result<Self, Self::Error> {
        if !record.analysis.ends_with(":PHRASE") {
            bail!("Tried to convert a non-phrase record into Phrase");
        }

        let Record {
            id,
            root: _,
            root_ntws: _,
            root_1: _,
            lemma: _,
            lemma_search: _,
            form,
            lemma_bw: _,
            form_bw,
            caphipp,
            analysis: _,
            gloss,
            gloss_msa,
            example_usage,
            notes,
        } = record;

        let (glosses, _auto) = Definition::parse_glosses(&gloss);

        Ok(Self {
            id,
            form: form.as_str().into(),
            form_bw: form_bw.as_str().into(),
            caphipp: caphipp.as_str().into(),
            glosses,
            gloss_msa: gloss_msa.as_str().into(),
            example_usage: example_usage.as_str().into(),
            notes: notes.as_str().into(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct Lemma {
    pub root: Root,
    pub root_1: NormalizedInternedString,

    pub lemma: NormalizedInternedString,
    pub lemma_search: NormalizedInternedString,
    pub lemma_bw: NormalizedInternedString,

    pub definitions: Vec<Definition>,
    pub phrases: Vec<Phrase>,
}

impl Lemma {
    pub(crate) fn lowest_id(&self) -> u32 {
        self.definitions
            .iter()
            .map(|def| def.id)
            .chain(self.phrases.iter().map(|ph| ph.id))
            .min()
            .unwrap_or(u32::MAX)
    }

    /// Check whether a query matches this lemma.
    fn matches(&self, lexicon: &Lexicon, query: &query::Query) -> (bool, f64) {
        match query {
            query::Query::Leaf(leaf) => {
                let (leaf_match, tf_idf) = self.matches_leaf(lexicon, leaf);
                let matches = leaf_match
                    || self.definitions.iter().any(|def| def.matches(leaf))
                    || self.phrases.iter().any(|ph| ph.matches(leaf));
                (matches, tf_idf)
            }
            query::Query::Operator { op, lhs, rhs } => {
                let (lhs, lhs_tf_idf) = self.matches(lexicon, lhs);
                match op {
                    query::Operator::And => {
                        if let Some((true, rhs_tf_idf)) = lhs.then(|| self.matches(lexicon, rhs)) {
                            (true, lhs_tf_idf + rhs_tf_idf)
                        } else {
                            (false, 0.0)
                        }
                    }
                    query::Operator::Or => {
                        let (rhs, rhs_tf_idf) = self.matches(lexicon, rhs);
                        let tf_idf =
                            if lhs { lhs_tf_idf } else { 0.0 } + if rhs { rhs_tf_idf } else { 0.0 };
                        (lhs || rhs, tf_idf)
                    }
                }
            }
        }
    }

    /// Match a query leaf against the lemma-wide fields.
    fn matches_leaf(&self, lexicon: &Lexicon, leaf: &query::Leaf) -> (bool, f64) {
        let (matches, term) = match leaf {
            query::Leaf::Term { term } => {
                let matches = term.matches(&self.root)
                    || term.matches(&self.root_1)
                    || term.matches(&self.lemma)
                    || term.matches(&self.lemma_search);
                (matches, term)
            }
            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Term => {
                    let matches = term.matches(&self.lemma)
                        || term.matches(&self.lemma_search)
                        || term.matches(&self.lemma_bw);
                    (matches, term)
                }
                _ => (false, term),
            },
        };

        (matches, lexicon.tf_idf(self, term))
    }

    /// Merge a new lemma into an existing one.
    fn merge(&mut self, lemma: Lemma) -> Result<()> {
        ensure!(self.root == lemma.root);
        ensure!(self.root_1 == lemma.root_1);
        ensure!(self.lemma == lemma.lemma);
        ensure!(self.lemma_search == lemma.lemma_search);
        ensure!(self.lemma_bw == lemma.lemma_bw);

        self.definitions.extend(lemma.definitions);
        self.phrases.extend(lemma.phrases);

        Ok(())
    }
}

impl TryFrom<Record> for Lemma {
    type Error = anyhow::Error;

    fn try_from(mut record: Record) -> Result<Self> {
        let root = record.root.take().unwrap();
        let root_ntws = record.root_ntws.take().unwrap_or_default();
        let root_1 = record.root_1.take().unwrap();
        let lemma = record.lemma.take().unwrap();
        let lemma_search = record.lemma_search.take().unwrap();
        let lemma_bw = record.lemma_bw.take().unwrap();

        let mut lemma = Lemma {
            root: Root::new(root, root_ntws),
            root_1: root_1.as_str().into(),
            lemma: lemma.as_str().into(),
            lemma_search: lemma_search.as_str().into(),
            lemma_bw: lemma_bw.as_str().into(),

            definitions: Default::default(),
            phrases: Default::default(),
        };

        if record.analysis.ends_with(":PHRASE") {
            lemma
                .phrases
                .push(Phrase::try_from(record).context("Failed to convert Record to Phrase")?);
        } else {
            lemma
                .definitions
                .push(Definition::try_from(record).context("Failed to convert Record to Phrase")?);
        }

        Ok(lemma)
    }
}

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

        // Group the raw records by root, lemma, and part of speech.
        //
        // Also separate definitions from phrases.
        let mut lemmas =
            HashMap::<(Root, NormalizedInternedString, NormalizedInternedString), Lemma>::new();
        for record in reader.deserialize::<Record>() {
            let record = record.context("Failed to deserialize lexicon record")?;
            let Some(record) = patch_record(record).context("Failed to patch record")? else {
                continue;
            };

            let pos = record.analysis.split(':').next().unwrap().into();

            let lemma = Lemma::try_from(record).context("Failed to convert record to lemma")?;
            let lemma_id = lemma.lowest_id();

            match lemmas.entry((lemma.root.clone(), lemma.lemma.clone(), pos)) {
                HMEntry::Occupied(occupied) => {
                    occupied.into_mut().merge(lemma).with_context(|| {
                        format!("Failed to merge new lemma into existing lemma for {lemma_id}",)
                    })?;
                }
                HMEntry::Vacant(vacant) => {
                    vacant.insert_entry(lemma);
                }
            };
        }

        // Sort definitions and phrases by part of speech and ID.
        for lemma in lemmas.values_mut() {
            lemma
                .definitions
                .sort_by_key(|entry| (entry.custom.pos, entry.id));
            lemma.phrases.sort_by_key(|entry| entry.id);
        }

        // Sort lemmas by lowest ID among entries and phrases to have a consistent order.
        //
        // This is only necessary since we don't perform the ranking step of a proper search
        // engine.
        let mut lemmas = lemmas.into_values().collect::<Vec<_>>();
        lemmas.sort_by_key(Lemma::lowest_id);

        let mut idf = InverseDocumentFrequencies::new();
        let mut term_freqs = HashMap::new();
        for lemma in &lemmas {
            let tfs = DocumentTermFrequencies::from(lemma);
            idf.add_document(&tfs);
            term_freqs.insert(lemma.lowest_id(), tfs);
        }

        Ok(Self {
            lemmas: Arc::new(lemmas),
            inverse_doc_freqs: Arc::new(idf),
            term_freqs: Arc::new(term_freqs),
        })
    }

    pub(crate) fn search(&self, query: &query::Query) -> impl Iterator<Item = (&Lemma, f64)> {
        self.lemmas.iter().filter_map(|lemma| {
            let (matches, tf_idf) = lemma.matches(self, query);
            matches.then_some((lemma, tf_idf))
        })
    }

    fn tf_idf(&self, lemma: &Lemma, term: &query::Term) -> f64 {
        let term_freqs = self.term_freqs.get(&lemma.lowest_id()).unwrap();

        term.to_terms()
            .map(|term| {
                let tf = term_freqs.term_frequency(&term);
                let idf = self.inverse_doc_freqs.idf(&term);

                tf * idf
            })
            .sum()
    }
}

/// Apply patches to records with errors.
fn patch_record(mut record: Record) -> Result<Option<Record>> {
    match record.id {
        2737 => {
            ensure!(record.root_ntws.unwrap() == "ب.ي.ر");
            record.root_ntws = Some("ب.ي.ر.و".to_compact_string());
        }

        // Duplicates of 29603, 29605, 29612 but with different orders of the fatah and shadda in the
        // lemma.
        29598 | 29607 | 29614 => {
            ensure!(record.lemma_bw.unwrap() == "laq~aT");
            return Ok(None);
        }
        _ => {}
    }

    match record.analysis.as_str() {
        "NOUN:PL" => {
            record.analysis.pop();
        }
        "NOUN:SF" => {
            record.analysis = "NOUN:FS".to_compact_string();
        }
        _ => {}
    }

    Ok(Some(record))
}
