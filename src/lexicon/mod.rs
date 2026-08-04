use std::{
    collections::{HashMap, hash_map::Entry as HMEntry},
    ops::Deref,
    sync::Arc,
};

use anyhow::{Context as _, Result, bail, ensure};
use compact_str::{CompactString, ToCompactString};

use crate::{
    lexicon::pos::PartOfSpeech,
    query::{self, MatchTerm as _},
    string::{InternedString, NormalizedString, SearchableString},
    tf_idf::{DocumentTermFrequencies, InverseDocumentFrequencies, ToTerms},
};

mod phonetics;
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
    Root(NormalizedString),
    NonTemplaticWordStem(NormalizedString),
}

impl Root {
    fn new(root: CompactString, root_ntws: CompactString) -> Self {
        let middledots = |c| if c == '.' { '·' } else { c };

        if root == "NTWS" {
            Self::NonTemplaticWordStem(NormalizedString::interned(
                root_ntws.chars().map(middledots),
            ))
        } else {
            Self::Root(NormalizedString::interned(root.chars().map(middledots)))
        }
    }
}

impl Deref for Root {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Root::Root(string) | Root::NonTemplaticWordStem(string) => string,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Transcription {
    pub caphipp: NormalizedString,
    pub bw: NormalizedString,
    /// List of IPA transcriptions for a given word.
    ///
    /// These are constructed from the CAPHI++ field.
    ///
    /// These intentionally use raw interned strings rather than `NormalizedString` because that
    /// type performs NKFC normalization which decomposes IPA modifier characters into the
    /// non-modifier equivalent, which renders incorrectly.
    pub ipa: Vec<InternedString>,
}

impl Transcription {
    fn new<T: AsRef<str>>(caphipp: T, bw: T) -> Result<Self> {
        Ok(Self {
            caphipp: NormalizedString::interned(caphipp.as_ref().chars()),
            bw: NormalizedString::interned(bw.as_ref().chars()),
            ipa: phonetics::caphipp_to_ipa(caphipp.as_ref())
                .context("Failed to convert CAPHI++ to IPA")?,
        })
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

    pub form: NormalizedString,
    pub transcription: Transcription,
    pub analysis: SearchableString,

    pub glosses: Vec<SearchableString>,
    pub gloss_msa: NormalizedString,

    pub example_usage: NormalizedString,
    pub notes: SearchableString,

    pub custom: Custom,
}

impl Definition {
    fn matches(&self, query: &query::Leaf) -> bool {
        match query {
            query::Leaf::Term { term } => {
                term.matches(&self.form)
                    || term.matches(&self.transcription.bw)
                    || self.glosses.iter().any(|gloss| term.matches(gloss))
            }
            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Lemma => {
                    term.matches(&self.form)
                        || term.matches(&self.transcription.bw)
                        || term.matches(&self.transcription.caphipp)
                }
                query::Qualifier::Analysis => term.matches(&self.analysis),
                query::Qualifier::Gloss => self.glosses.iter().any(|gloss| term.matches(gloss)),
            },
        }
    }

    /// Parse a gloss string into a vector of glosses, removing the auto-generated entry suffix if
    /// present.
    fn parse_glosses(gloss: &str) -> (Vec<SearchableString>, bool) {
        let stripped = gloss.strip_suffix("[auto]");
        let auto = stripped.is_some();

        let glosses: Vec<SearchableString> = stripped
            .unwrap_or(gloss)
            .replace("_", " ")
            .split(';')
            .map(|g| SearchableString::interned(g.trim().chars()))
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
            form: NormalizedString::interned(form.chars()),
            transcription: Transcription::new(caphipp, form_bw)
                .with_context(|| format!("Failed to parse transcriptions of record {id}"))?,
            analysis: SearchableString::interned(analysis.chars()),
            glosses,
            gloss_msa: NormalizedString::interned(gloss_msa.chars()),
            example_usage: NormalizedString::interned(example_usage.chars()),
            notes: SearchableString::interned(notes.chars()),

            custom: Custom { pos, auto },
        })
    }
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Phrase {
    pub id: u32,

    pub form: NormalizedString,
    pub transcription: Transcription,

    pub glosses: Vec<SearchableString>,
    pub gloss_msa: NormalizedString,

    pub example_usage: NormalizedString,
    pub notes: SearchableString,
}

impl Phrase {
    fn matches(&self, query: &query::Leaf) -> bool {
        match query {
            query::Leaf::Term { term } => {
                term.matches(&self.form)
                    || term.matches(&self.transcription.bw)
                    || self.glosses.iter().any(|gloss| term.matches(gloss))
            }
            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Lemma => {
                    term.matches(&self.form)
                        || term.matches(&self.transcription.bw)
                        || term.matches(&self.transcription.caphipp)
                }
                query::Qualifier::Analysis => false,
                query::Qualifier::Gloss => self.glosses.iter().any(|gloss| term.matches(gloss)),
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
            form: NormalizedString::interned(form.chars()),
            transcription: Transcription::new(caphipp, form_bw)
                .with_context(|| format!("Failed to parse transcriptions of record {id}"))?,
            glosses,
            gloss_msa: NormalizedString::interned(gloss_msa.chars()),
            example_usage: NormalizedString::interned(example_usage.chars()),
            notes: SearchableString::interned(notes.chars()),
        })
    }
}

#[derive(Debug)]
pub(crate) struct Lemma {
    pub root: Root,
    pub root_1: NormalizedString,

    pub lemma: NormalizedString,
    pub lemma_search: NormalizedString,
    pub lemma_bw: NormalizedString,

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
                    || term.matches(&self.lemma)
                    || term.matches(&self.lemma_search);
                (matches, term)
            }

            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Lemma => {
                    let matches = term.matches(&self.lemma)
                        || term.matches(&self.lemma_search)
                        || term.matches(&self.lemma_bw);
                    (matches, term)
                }
                query::Qualifier::Analysis | query::Qualifier::Gloss => (false, term),
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
            root_1: NormalizedString::interned(root_1.chars()),
            lemma: NormalizedString::interned(lemma.chars()),
            lemma_search: NormalizedString::interned(lemma_search.chars()),
            lemma_bw: NormalizedString::interned(lemma_bw.chars()),

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
        let mut lemmas = HashMap::<(Root, NormalizedString, NormalizedString), Lemma>::new();
        for record in reader.deserialize::<Record>() {
            let record = record.context("Failed to deserialize lexicon record")?;
            let Some(record) = patch_record(record).context("Failed to patch record")? else {
                continue;
            };

            let pos =
                NormalizedString::interned(record.analysis.split(':').next().unwrap().chars());

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

        // Invalid CAPHI++, II instead of || for alternate phonemes.
        15942 | 16642 | 16643 | 16644 | 16658 | 16659 | 16660 | 16661 | 17007 | 17008 | 25633
        | 25634 | 25716 | 26050 | 26054 | 26264 => {
            ensure!(record.caphipp.contains("II"));
            record.caphipp = record.caphipp.replace("II", "||").into();
        }
        // Same but in the gloss field.
        18000 => {
            ensure!(record.gloss.contains("II"));
            record.gloss = record.gloss.replace("II", "||").into();
        }
        // Same but in the notes field.
        25861 => {
            ensure!(record.notes.contains("II"));
            record.notes = record.notes.replace("II", "||").into();
        }

        // Apparent dupe of 31411 with "wealth (type)" instead of "wealth" as the gloss.
        31411 => {
            ensure!(record.lemma_bw.unwrap() == "maAl");
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
