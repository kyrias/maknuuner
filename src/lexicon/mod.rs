use std::{
    collections::{HashMap, hash_map::Entry as HMEntry},
    ops::Deref,
    sync::Arc,
};

use anyhow::{Context as _, Result, ensure};

use crate::{Str, lexicon::pos::PartOfSpeech, query};

pub(crate) mod pos;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) struct Record {
    id: u32,

    // These are `Option` so that `Lemma::merge` can take them and still call
    // `Entry::from(Record)`.
    root: Option<String>,
    root_ntws: Option<String>,
    root_1: Option<String>,
    lemma: Option<String>,
    lemma_search: Option<String>,
    lemma_bw: Option<String>,

    form: String,
    form_bw: String,
    #[serde(rename = "CAPHI++")]
    caphipp: String,
    analysis: String,
    gloss: String,
    gloss_msa: String,
    example_usage: String,
    notes: String,
    source: String,
    annotator: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) enum Root {
    Root(Str),
    NonTemplaticWordStem(Str),
}

impl Root {
    fn new(root: String, root_ntws: String) -> Self {
        if root == "NTWS" {
            Self::NonTemplaticWordStem(root_ntws.into())
        } else {
            Self::Root(root.into())
        }
    }
}

impl Deref for Root {
    type Target = Str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Root(root) => &root,
            Self::NonTemplaticWordStem(ntws) => &ntws,
        }
    }
}

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
#[derive(Debug)]
pub(crate) struct Entry {
    pub id: u32,

    pub form: Str,
    pub form_bw: Str,
    pub caphipp: Str,
    pub analysis: Str,

    pub glosses: Vec<Str>,
    pub gloss_msa: Str,

    pub example_usage: Str,
    pub notes: Str,
    pub source: Str,
    pub annotator: Str,

    pub custom: Custom,
}

impl Entry {
    fn matches(&self, query: &query::Leaf) -> bool {
        match query {
            query::Leaf::Term { term } => {
                term.matches(&self.form)
                    || term.matches(&self.form_bw)
                    || term.matches(&self.caphipp)
                    || term.matches(&self.analysis)
                    || self.glosses.iter().any(|gloss| term.matches(gloss))
                    || term.matches(&self.gloss_msa)
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
    fn parse_glosses(gloss: &str) -> (Vec<Str>, bool) {
        let stripped = gloss.strip_suffix("[auto]");
        let auto = stripped.is_some();

        let glosses: Vec<Str> = stripped
            .unwrap_or(gloss)
            .replace("_", " ")
            .split(';')
            .map(|g| g.trim().to_string().into())
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

impl From<Record> for Entry {
    fn from(record: Record) -> Self {
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
            source,
            annotator,
        } = record;

        let (glosses, auto) = Self::parse_glosses(&gloss);
        let pos = Self::parse_pos(id, &analysis);

        Self {
            id,
            form: form.into(),
            form_bw: form_bw.into(),
            caphipp: caphipp.into(),
            analysis: analysis.into(),
            glosses,
            gloss_msa: gloss_msa.into(),
            example_usage: example_usage.into(),
            notes: notes.into(),
            source: source.into(),
            annotator: annotator.into(),

            custom: Custom { pos, auto },
        }
    }
}

#[derive(Debug)]
pub(crate) struct Lemma {
    pub root: Root,
    pub root_1: Str,

    pub lemma: Str,
    pub lemma_search: Str,
    pub lemma_bw: Str,

    pub entries: Vec<Entry>,
    pub phrases: Vec<Entry>,
}

impl Lemma {
    /// Check whether a query matches this lemma.
    fn matches(&self, query: &query::Query) -> bool {
        match query {
            query::Query::Leaf(leaf) => {
                self.matches_leaf(leaf)
                    || self
                        .entries
                        .iter()
                        .chain(self.phrases.iter())
                        .any(|entry| entry.matches(leaf))
            }
            query::Query::Operator { op, lhs, rhs } => {
                let lhs = self.matches(lhs);
                match op {
                    query::Operator::And => lhs && self.matches(rhs),
                    query::Operator::Or => lhs || self.matches(rhs),
                }
            }
        }
    }

    /// Match a query leaf against the lemma-wide fields.
    fn matches_leaf(&self, leaf: &query::Leaf) -> bool {
        match leaf {
            query::Leaf::Term { term } => {
                term.matches(&*self.root)
                    || term.matches(&self.root_1)
                    || term.matches(&self.lemma)
                    || term.matches(&self.lemma_search)
                    || term.matches(&self.lemma_bw)
            }
            query::Leaf::Qualified { qualifier, term } => match qualifier {
                query::Qualifier::Term => {
                    term.matches(&self.lemma)
                        || term.matches(&self.lemma_search)
                        || term.matches(&self.lemma_bw)
                }
                _ => false,
            },
        }
    }

    /// Merge a record into an existing lemma.
    fn merge(&mut self, record: Record) -> Result<()> {
        let Record {
            id,
            root,
            root_ntws,
            root_1,
            lemma,
            lemma_search,
            form,
            lemma_bw,
            form_bw,
            caphipp,
            analysis,
            gloss,
            gloss_msa,
            example_usage,
            notes,
            source,
            annotator,
        } = record;

        let root = root.unwrap();
        if root == "NTWS" {
            ensure!(self.root.raw == root_ntws.as_deref().unwrap());
        } else {
            ensure!(self.root.raw == root.as_str());
        }
        ensure!(Some(&self.root_1.raw) == root_1.as_ref());
        ensure!(Some(&self.lemma.raw) == lemma.as_ref());
        ensure!(Some(&self.lemma_search.raw) == lemma_search.as_ref());
        ensure!(Some(&self.lemma_bw.raw) == lemma_bw.as_ref());

        let is_phrase = analysis.ends_with(":PHRASE");
        let (glosses, auto) = Entry::parse_glosses(&gloss);
        let pos = Entry::parse_pos(id, &analysis);

        let entry = Entry {
            id,
            form: form.into(),
            form_bw: form_bw.into(),
            caphipp: caphipp.into(),
            analysis: analysis.into(),
            glosses,
            gloss_msa: gloss_msa.into(),
            example_usage: example_usage.into(),
            notes: notes.into(),
            source: source.into(),
            annotator: annotator.into(),

            custom: Custom { pos, auto },
        };

        if is_phrase {
            self.phrases.push(entry);
        } else {
            self.entries.push(entry);
        };

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
            root_1: root_1.into(),
            lemma: lemma.into(),
            lemma_search: lemma_search.into(),
            lemma_bw: lemma_bw.into(),

            entries: Default::default(),
            phrases: Default::default(),
        };

        if record.analysis.ends_with(":PHRASE") {
            lemma.phrases.push(Entry::from(record));
        } else {
            lemma.entries.push(Entry::from(record));
        };

        Ok(lemma)
    }
}

pub(crate) struct Lexicon {
    pub lemmas: Arc<Vec<Lemma>>,
}

impl Lexicon {
    pub(super) fn new() -> Result<Self> {
        const LEXICON: &'static str = include_str!("../../maknuune-v1.0.1.tsv");

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b'\t')
            .from_reader(LEXICON.as_bytes());

        // Group the raw records by lemma and part of speech.
        //
        // Also separate definitions from phrases.
        let mut lemmas = HashMap::<(Root, String, String), Lemma>::new();
        for record in reader.deserialize::<Record>() {
            let record = patch_record(record.context("Failed to deserialize lexicon record")?)
                .context("Failed to patch record")?;

            let lemma_key = (
                Root::new(
                    record.root.clone().unwrap(),
                    record.root_ntws.clone().unwrap_or_default(),
                ),
                record.lemma.clone().unwrap(),
                record.analysis.split(':').next().unwrap().to_string(),
            );

            match lemmas.entry(lemma_key) {
                HMEntry::Occupied(occupied) => {
                    let lemma = occupied.into_mut();
                    lemma.merge(record).with_context(|| {
                        format!(
                            "Failed to merge record into existing lemma for {}",
                            lemma.lemma_bw.raw,
                        )
                    })?;
                }
                HMEntry::Vacant(vacant) => {
                    vacant.insert_entry(
                        Lemma::try_from(record).context("Failed to convert record to lemma")?,
                    );
                }
            };
        }

        // Sort entries by part of speech and ID.
        for lemma in lemmas.values_mut() {
            lemma
                .entries
                .sort_by_key(|entry| (entry.custom.pos, entry.id));
            lemma
                .phrases
                .sort_by_key(|entry| (entry.custom.pos, entry.id));
        }

        // Sort lemmas by lowest ID among entries and phrases to have a consistent order.
        //
        // This is only necessary since we don't perform the ranking step of a proper search
        // engine.
        let mut lemmas = lemmas.into_values().collect::<Vec<_>>();
        lemmas.sort_by_key(|lemma| {
            lemma
                .entries
                .iter()
                .chain(lemma.phrases.iter())
                .map(|entry| entry.id)
                .min()
                .expect("All lemmas have at least one entry or phrase")
        });

        Ok(Self {
            lemmas: Arc::new(lemmas),
        })
    }

    pub(crate) fn search(&self, query: &query::Query) -> impl Iterator<Item = &Lemma> {
        self.lemmas.iter().filter(|lemma| lemma.matches(query))
    }
}

/// Apply patches to records with errors.
fn patch_record(mut record: Record) -> Result<Record> {
    if record.id == 2737 {
        ensure!(record.root_ntws.unwrap() == "ب.ي.ر");
        record.root_ntws = Some("ب.ي.ر.و".to_string());
    }

    match record.analysis.as_str() {
        "NOUN:PL" => {
            record.analysis.pop();
        }
        "NOUN:SF" => {
            record.analysis = "NOUN:FS".to_string();
        }
        _ => {}
    }

    Ok(record)
}
