use std::{fmt::Debug, ops::Not, str::FromStr};

use anyhow::{Context, Error, bail};

pub(crate) mod noun {

    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum NounFeature {
        Singular,
        MasculineSingular,
        FeminineSingular,
        Dual,
        Plural,
        MasculinePlural,
        FemininePlural,
    }

    impl FromStr for NounFeature {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let s = s
                .strip_prefix(':')
                .context("NounFeature didn't start with colon")?;

            let feature = match s {
                "D" => Self::Dual,
                "FP" => Self::FemininePlural,
                "FS" => Self::FeminineSingular,
                "MP" => Self::MasculinePlural,
                "MS" => Self::MasculineSingular,
                "P" => Self::Plural,
                "S" => Self::Singular,
                _ => bail!("Unknown NounFeature {s:?}"),
            };

            Ok(feature)
        }
    }

    impl Debug for NounFeature {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let s = match self {
                NounFeature::Singular => "Singular",
                NounFeature::MasculineSingular => "Masculine Singular",
                NounFeature::FeminineSingular => "Feminine Singular",
                NounFeature::Dual => "Dual",
                NounFeature::Plural => "Plural",
                NounFeature::MasculinePlural => "Masculine Plural",
                NounFeature::FemininePlural => "Feminine Plural",
            };
            f.write_str(s)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum Noun {
        Plain(Option<NounFeature>),
        /// Active participle deverbal noun
        Active(Option<NounFeature>),
        /// Passive participle deverbal noun
        Passive(Option<NounFeature>),
        /// Proper nouns
        Proper(Option<NounFeature>),
        /// Cardinal numbers
        Number(Option<NounFeature>),
        /// Noun quantifier
        Quantifier(Option<NounFeature>),
    }

    impl FromStr for Noun {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let idx = s.find(':').unwrap_or(s.len());
            let (prefix, suffix) = s.split_at(idx);

            let constructor = match prefix {
                "" => Self::Plain,
                "_ACT" => Self::Active,
                "_PASS" => Self::Passive,
                "_PROP" => Self::Proper,
                "_NUM" => Self::Number,
                "_QUANT" => Self::Quantifier,
                _ => bail!("Unknown Noun sub-type {prefix:?}"),
            };

            let feature = suffix
                .is_empty()
                .not()
                .then(|| suffix.parse())
                .transpose()?;
            Ok(constructor(feature))
        }
    }

    impl Debug for Noun {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let (s, nf) = match self {
                Noun::Plain(noun_feature) => ("", noun_feature),
                Noun::Active(noun_feature) => (" Active", noun_feature),
                Noun::Passive(noun_feature) => (" Passive", noun_feature),
                Noun::Proper(noun_feature) => (" Proper", noun_feature),
                Noun::Number(noun_feature) => (" Number", noun_feature),
                Noun::Quantifier(noun_feature) => (" Quantifier", noun_feature),
            };
            f.write_str("Noun")?;
            f.write_str(s)?;
            if let Some(nf) = nf {
                write!(f, " // {:?}", nf)?;
            }

            Ok(())
        }
    }
}

pub(crate) mod verb {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum VerbFeature {
        Perfective,
        Command,
        Imperfective,
    }

    impl FromStr for VerbFeature {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let s = s
                .strip_prefix(':')
                .context("VerbFeature didn't start with colon")?;

            let feature = match s {
                "C" => Self::Command,
                "I" => Self::Imperfective,
                "P" => Self::Perfective,
                _ => bail!("Unknown VerbFeature {s:?}"),
            };

            Ok(feature)
        }
    }

    impl Debug for VerbFeature {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let s = match self {
                VerbFeature::Perfective => "Perfective",
                VerbFeature::Command => "Command",
                VerbFeature::Imperfective => "Imperfective",
            };
            f.write_str(s)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum Verb {
        Plain(VerbFeature),
        /// Non-inflectional verb, also called frozen verbs
        Nominal,
        /// Pseudo verb
        Pseudo,
    }

    impl FromStr for Verb {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let idx = s.find(':').unwrap_or(s.len());
            let (prefix, suffix) = s.split_at(idx);

            let verb = match prefix {
                "" => Self::Plain(suffix.parse()?),
                "_NOM" => Self::Nominal,
                "_PSEUDO" => Self::Pseudo,
                _ => bail!("Unknown Verb sub-type {prefix:?}"),
            };

            Ok(verb)
        }
    }

    impl Debug for Verb {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let (s, vf) = match self {
                Verb::Plain(verb_feature) => ("", Some(verb_feature)),
                Verb::Nominal => (" Nominal", None),
                Verb::Pseudo => (" Pseudo", None),
            };
            f.write_str("Verb")?;
            f.write_str(s)?;
            if let Some(vf) = vf {
                write!(f, " // {:?}", vf)?;
            }

            Ok(())
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PartOfSpeech {
    Noun(noun::Noun),
    Verb(verb::Verb),
}

impl FromStr for PartOfSpeech {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let idx = s.find(|c| ['_', '/', ':'].contains(&c)).unwrap_or(s.len());
        let (prefix, suffix) = s.split_at(idx);

        let pos = match prefix {
            "NOUN" => Self::Noun(suffix.parse()?),
            "VERB" => Self::Verb(suffix.parse()?),
            _ => bail!("Unknown PartOfSpeech {prefix:?}"),
        };

        Ok(pos)
    }
}

impl Debug for PartOfSpeech {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartOfSpeech::Noun(noun) => write!(f, "{:?}", noun),
            PartOfSpeech::Verb(verb) => write!(f, "{:?}", verb),
        }
    }
}
