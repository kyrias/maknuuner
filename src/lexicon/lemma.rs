use std::{fmt::Debug, ops::Deref};

use anyhow::{Context, Result, bail, ensure};
use compact_str::CompactString;
use itertools::Itertools;
use smallvec::SmallVec;

use crate::{
    lexicon::{phonetics::caphipp_to_ipa, pos::PartOfSpeech},
    string::{
        CaseFoldedNfkcNormalizedString, NfcNormalizedString, NonFoldedNfkcNormalizedString,
        SearchableString,
    },
};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) struct DatasetEntry {
    pub(super) id: u32,

    // These are `Option` so that `Lemma::merge` can take them and still call
    // `Definition/Phrase::try_from(DatasetEntry)`.
    pub(super) root: Option<CompactString>,
    pub(super) root_ntws: Option<CompactString>,
    pub(super) root_1: char,
    pub(super) lemma: Option<CompactString>,
    pub(super) lemma_search: Option<CompactString>,
    pub(super) lemma_bw: Option<CompactString>,

    pub(super) form: CompactString,
    pub(super) form_bw: CompactString,
    #[serde(rename = "CAPHI++")]
    pub(super) caphipp: CompactString,
    pub(super) analysis: CompactString,
    pub(super) gloss: CompactString,
    pub(super) gloss_msa: CompactString,
    pub(super) example_usage: CompactString,
    pub(super) notes: CompactString,
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) enum Root {
    Root(SearchableString),
    NonTemplaticWordStem(SearchableString),
}

impl Root {
    fn new(root: CompactString, root_ntws: CompactString) -> Self {
        let middledots = |c| if c == '.' { '·' } else { c };

        if root == "NTWS" {
            Self::NonTemplaticWordStem(SearchableString::case_folded(
                root_ntws.chars().map(middledots),
            ))
        } else {
            Self::Root(SearchableString::case_folded(root.chars().map(middledots)))
        }
    }
}

impl Deref for Root {
    type Target = SearchableString;

    fn deref(&self) -> &Self::Target {
        match self {
            Root::Root(string) | Root::NonTemplaticWordStem(string) => string,
        }
    }
}

impl Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Root({:?})", self.displayable.as_str())
    }
}

#[derive(Debug)]
pub(crate) struct Lemma {
    pub lowest_id: u32,

    pub root: Root,
    pub root_1: char,

    pub lemma: SearchableString,
    pub lemma_search: SearchableString,
    pub lemma_bw: SearchableString,

    pub definitions: SmallVec<[Definition; 1]>,
    pub phrases: SmallVec<[Phrase; 1]>,
}

impl Lemma {
    /// Merge a new lemma into an existing one.
    pub(super) fn merge(&mut self, lemma: Lemma) -> Result<()> {
        ensure!(self.root == lemma.root);
        ensure!(self.root_1 == lemma.root_1);
        ensure!(self.lemma == lemma.lemma);
        ensure!(self.lemma_search == lemma.lemma_search);
        ensure!(self.lemma_bw == lemma.lemma_bw);

        self.definitions.extend(lemma.definitions);
        self.phrases.extend(lemma.phrases);

        self.lowest_id = self
            .definitions
            .iter()
            .map(|def| def.id)
            .chain(self.phrases.iter().map(|ph| ph.id))
            .min()
            .unwrap();

        Ok(())
    }
}

impl TryFrom<DatasetEntry> for Lemma {
    type Error = anyhow::Error;

    fn try_from(mut entry: DatasetEntry) -> Result<Self> {
        let root = entry.root.take().unwrap();
        let root_ntws = entry.root_ntws.take().unwrap_or_default();
        let root_1 = entry.root_1;
        let lemma = entry.lemma.take().unwrap();
        let lemma_search = entry.lemma_search.take().unwrap();
        let lemma_bw = entry.lemma_bw.take().unwrap();

        let mut lemma = Lemma {
            lowest_id: entry.id,
            root: Root::new(root, root_ntws),
            root_1,
            lemma: SearchableString::case_folded(lemma.chars()),
            lemma_search: SearchableString::case_folded(lemma_search.chars()),
            lemma_bw: SearchableString::non_folded(lemma_bw.chars()),

            definitions: Default::default(),
            phrases: Default::default(),
        };

        if entry.analysis.ends_with(":PHRASE") {
            lemma
                .phrases
                .push(Phrase::try_from(entry).context("Failed to convert DatasetEntry to Phrase")?);
        } else {
            lemma.definitions.push(
                Definition::try_from(entry).context("Failed to convert DatasetEntry to Phrase")?,
            );
        }

        Ok(lemma)
    }
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Definition {
    pub id: u32,

    pub form: SearchableString,
    pub transcription: Transcription,
    pub analysis: CaseFoldedNfkcNormalizedString,
    /// Parsed form of the analysis field.
    pub pos: Option<PartOfSpeech>,

    pub glosses_english: SmallVec<[SearchableString; 1]>,
    pub glosses_msa: SmallVec<[SearchableString; 1]>,

    pub example_usage: NfcNormalizedString,
    pub notes: NfcNormalizedString,
}

impl Definition {
    /// Parse a gloss string into a vector of glosses, removing the auto-generated entry suffix if
    /// present.
    ///
    /// The GLOSS field contains a semicolon-separated list of glosses, but an entry may contain
    /// semicolons within a quoted string.
    fn parse_glosses(gloss: &str) -> SmallVec<[SearchableString; 1]> {
        let mut out = SmallVec::new();

        let mut iter = gloss
            .strip_suffix("[auto]")
            .unwrap_or(gloss)
            .chars()
            .peekable();
        let mut tmp = CompactString::default();
        loop {
            tmp.extend(iter.peeking_take_while(|c| ![';', '"'].contains(c)));

            match iter.next() {
                Some(';') | None => {
                    let string = SearchableString::case_folded(
                        std::mem::take(&mut tmp)
                            .chars()
                            .map(|c| if c == '_' { ' ' } else { c }),
                    );

                    out.push(string);

                    if iter.peek().is_none() {
                        break;
                    }
                }

                Some('"') => {
                    tmp.push('"');
                    tmp.extend((&mut iter).take_while_inclusive(|c| !['"'].contains(c)));
                }

                _ => {
                    unreachable!()
                }
            }
        }

        out
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
                println!("Parsing POS failed for entry {id}: {err:#?}");
                None
            }
        }
    }
}

impl TryFrom<DatasetEntry> for Definition {
    type Error = anyhow::Error;

    fn try_from(entry: DatasetEntry) -> Result<Self, Self::Error> {
        if entry.analysis.ends_with(":PHRASE") {
            bail!("Tried to convert a phrase entry into Phrase");
        }

        let DatasetEntry {
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
        } = entry;

        let glosses = Self::parse_glosses(&gloss);
        let glosses_msa = gloss_msa
            .split('#')
            .map(|g| g.trim())
            .filter(|g| !g.is_empty())
            .map(|g| SearchableString::non_folded(g.chars()))
            .collect();
        let pos = Self::parse_pos(id, &analysis);

        Ok(Self {
            id,
            form: SearchableString::case_folded(form.chars()),
            transcription: Transcription::new(caphipp, form_bw)
                .with_context(|| format!("Failed to parse transcriptions of entry {id}"))?,
            analysis: CaseFoldedNfkcNormalizedString::interned(analysis.chars()),
            pos,
            glosses_english: glosses,
            glosses_msa,
            example_usage: NfcNormalizedString::interned(example_usage.chars()),
            notes: NfcNormalizedString::interned(notes.chars()),
        })
    }
}

#[derive(Debug)]
pub(crate) struct Transcription {
    pub caphipp: NonFoldedNfkcNormalizedString,
    pub bw: NonFoldedNfkcNormalizedString,
    /// List of IPA transcriptions for a given word.
    ///
    /// These are automatically constructed from the CAPHI++ field.
    pub ipa: SmallVec<[NfcNormalizedString; 1]>,
}

impl Transcription {
    fn new<T: AsRef<str>>(caphipp: T, bw: T) -> Result<Self> {
        Ok(Self {
            // Both of these have case-sensitive semantics.
            caphipp: NonFoldedNfkcNormalizedString::interned(caphipp.as_ref().chars()),
            bw: NonFoldedNfkcNormalizedString::interned(bw.as_ref().chars()),

            ipa: caphipp_to_ipa(caphipp.as_ref()).context("Failed to convert CAPHI++ to IPA")?,
        })
    }
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct Phrase {
    pub id: u32,

    pub form: SearchableString,
    pub transcription: Transcription,

    pub glosses_english: SmallVec<[SearchableString; 1]>,
    pub glosses_msa: SmallVec<[SearchableString; 1]>,

    pub example_usage: NfcNormalizedString,
    pub notes: NfcNormalizedString,
}

impl TryFrom<DatasetEntry> for Phrase {
    type Error = anyhow::Error;

    fn try_from(entry: DatasetEntry) -> Result<Self, Self::Error> {
        if !entry.analysis.ends_with(":PHRASE") {
            bail!("Tried to convert a non-phrase entry into Phrase");
        }

        let DatasetEntry {
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
        } = entry;

        let glosses = Definition::parse_glosses(&gloss);
        let glosses_msa = gloss_msa
            .split('#')
            .map(|g| g.trim())
            .filter(|g| !g.is_empty())
            .map(|g| SearchableString::non_folded(g.chars()))
            .collect();

        Ok(Self {
            id,
            form: SearchableString::case_folded(form.chars()),
            transcription: Transcription::new(caphipp, form_bw)
                .with_context(|| format!("Failed to parse transcriptions of entry {id}"))?,
            glosses_english: glosses,
            glosses_msa,
            example_usage: NfcNormalizedString::interned(example_usage.chars()),
            notes: NfcNormalizedString::interned(notes.chars()),
        })
    }
}
