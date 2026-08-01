use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::{Context as _, Result};

use crate::{
    Str,
    query::{Operator, Qualifier, Query},
};

pub(crate) mod pos;

#[derive(Debug)]
pub(crate) enum Root {
    Root(Str),
    NonTemplaticWordStem(Str),
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

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) struct RawRecord {
    id: u32,
    root: String,
    root_ntws: String,
    root_1: String,
    lemma: String,
    lemma_search: String,
    form: String,
    lemma_bw: String,
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

#[derive(Debug)]
pub(crate) struct Custom {
    /// Parsed form of the analysis field.
    pub pos: Option<pos::PartOfSpeech>,

    /// Whether this entry was automatically generated from another one.
    ///
    /// This is designated in the TSV dataset by `[auto]` being appended to the gloss field.
    pub auto: bool,
}

#[derive(Debug)]
pub(crate) struct Record {
    pub id: u32,
    pub root: Root,
    pub root_1: Str,
    pub lemma: Str,
    pub lemma_search: Str,
    pub form: Str,
    pub lemma_bw: Str,
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

impl TryFrom<RawRecord> for Record {
    type Error = anyhow::Error;

    fn try_from(value: RawRecord) -> Result<Self> {
        // TODO: This is a huge hack to be able to more easily incrementally start parsing the
        //       analysis field.
        let pos = if value.analysis.starts_with("NOUN") || value.analysis.starts_with("VERB") {
            match value.analysis.parse() {
                Ok(pos) => Some(pos),
                Err(err) => {
                    println!("Parsing POS failed: {err:#?}");
                    dbg!(&value);
                    None
                }
            }
        } else {
            None
        };

        let RawRecord {
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
        } = value;

        let stripped = gloss.strip_suffix("[auto]");
        let auto = stripped.is_some();
        let glosses: Vec<Str> = stripped
            .unwrap_or(&gloss)
            .replace("_", " ")
            .split(';')
            .map(|g| g.trim().to_string().into())
            .collect();

        Ok(Self {
            id,
            root: if root == "NTWS" {
                Root::NonTemplaticWordStem(root_ntws.into())
            } else {
                Root::Root(root.into())
            },
            root_1: root_1.into(),
            lemma: lemma.into(),
            lemma_search: lemma_search.into(),
            form: form.into(),
            lemma_bw: lemma_bw.into(),
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
        })
    }
}

trait QueryExt {
    fn matches(&self, record: &Record) -> bool;
}

impl QueryExt for Query {
    fn matches(&self, record: &Record) -> bool {
        match self {
            Query::Term { term } => {
                term.matches(&*record.root)
                    || term.matches(&record.root_1)
                    || term.matches(&record.lemma)
                    || term.matches(&record.lemma_search)
                    || term.matches(&record.form)
                    || term.matches(&record.lemma_bw)
                    || term.matches(&record.form_bw)
                    || term.matches(&record.caphipp)
                    || term.matches(&record.analysis)
                    || record.glosses.iter().any(|gloss| term.matches(gloss))
                    || term.matches(&record.gloss_msa)
            }
            Query::Qualified { qualifier, term } => match qualifier {
                Qualifier::Term => {
                    term.matches(&record.lemma)
                        || term.matches(&record.lemma_search)
                        || term.matches(&record.lemma_bw)
                        || term.matches(&record.gloss_msa)
                        || term.matches(&record.caphipp)
                }
                Qualifier::Analysis => term.matches(&record.analysis),
                Qualifier::Gloss => {
                    record.glosses.iter().any(|gloss| term.matches(gloss))
                        || term.matches(&record.gloss_msa)
                }
            },
            Query::Operator { op, lhs, rhs } => {
                let lhs = lhs.matches(record);
                match op {
                    Operator::And => lhs && rhs.matches(record),
                    Operator::Or => lhs || rhs.matches(record),
                }
            }
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct Lemma {
    pub entries: Vec<Record>,
    pub phrases: Vec<Record>,
}

impl Lemma {
    fn matches(&self, query: &Query) -> bool {
        match query {
            Query::Term { .. } => {
                self.entries.iter().any(|entry| query.matches(entry))
                    || self.phrases.iter().any(|entry| query.matches(entry))
            }
            Query::Qualified { .. } => {
                self.entries.iter().any(|entry| query.matches(entry))
                    || self.phrases.iter().any(|entry| query.matches(entry))
            }
            Query::Operator { op, lhs, rhs } => {
                let lhs = self.matches(lhs);
                match op {
                    Operator::And => lhs && self.matches(rhs),
                    Operator::Or => lhs || self.matches(rhs),
                }
            }
        }
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

        let mut lemmas = HashMap::<(String, String), Lemma>::new();
        for record in reader.deserialize::<RawRecord>() {
            let record = record.context("Failed to deserialize lexicon record")?;
            let pos = record.analysis.split(':').next().unwrap().to_string();

            let key = (record.lemma.clone(), pos);
            let lemma = lemmas.entry(key).or_default();

            if record.analysis.ends_with(":PHRASE") {
                lemma
                    .phrases
                    .push(record.try_into().context("Failed to convert record")?);
            } else {
                lemma
                    .entries
                    .push(record.try_into().context("Failed to convert record")?);
            }
        }

        // Sort enries
        for lemma in lemmas.values_mut() {
            lemma
                .entries
                .sort_by_key(|entry| (entry.custom.pos, entry.id));
            lemma
                .phrases
                .sort_by_key(|entry| (entry.custom.pos, entry.id));
        }

        // Sort lemmas by lowest ID among entries and phrases.
        let mut lemmas = lemmas.into_values().collect::<Vec<_>>();
        lemmas.sort_by_key(|lemma| {
            let min_entry_id = lemma
                .entries
                .iter()
                .map(|entry| entry.id)
                .min()
                .unwrap_or(u32::MAX);
            let min_phrase_id = lemma
                .phrases
                .iter()
                .map(|entry| entry.id)
                .min()
                .unwrap_or(u32::MAX);
            min_entry_id.min(min_phrase_id)
        });

        Ok(Self {
            lemmas: Arc::new(lemmas),
        })
    }

    pub(crate) fn search(&self, query: &Query) -> impl Iterator<Item = &Lemma> {
        self.lemmas.iter().filter(|lemma| lemma.matches(query))
    }
}
