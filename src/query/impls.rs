use crate::{
    lexicon::{Definition, Lemma, Lexicon, Phrase},
    query::{Leaf, Matches, Operator, Qualifier, Query},
};

impl Matches<Lemma> for Query {
    type Result = (bool, f64);

    fn matches(&self, lexicon: &Lexicon, value: &Lemma) -> Self::Result {
        match self {
            Query::Leaf(leaf) => {
                let (leaf_match, tf_idf) = leaf.matches(lexicon, value);
                let matches = leaf_match
                    || value
                        .definitions
                        .iter()
                        .any(|def| leaf.matches(lexicon, def))
                    || value.phrases.iter().any(|ph| leaf.matches(lexicon, ph));
                (matches, tf_idf)
            }
            Query::Operator { op, lhs, rhs } => {
                let (lhs, lhs_tf_idf) = lhs.matches(lexicon, value);
                match op {
                    Operator::And => {
                        if let Some((true, rhs_tf_idf)) = lhs.then(|| rhs.matches(lexicon, value)) {
                            (true, lhs_tf_idf + rhs_tf_idf)
                        } else {
                            (false, 0.0)
                        }
                    }
                    Operator::Or => {
                        let (rhs, rhs_tf_idf) = rhs.matches(lexicon, value);
                        let tf_idf =
                            if lhs { lhs_tf_idf } else { 0.0 } + if rhs { rhs_tf_idf } else { 0.0 };
                        (lhs || rhs, tf_idf)
                    }
                }
            }
        }
    }
}

impl Matches<Lemma> for Leaf {
    type Result = (bool, f64);

    fn matches(&self, lexicon: &Lexicon, value: &Lemma) -> Self::Result {
        let (matches, term) = match self {
            Leaf::Term { term } => {
                let matches = term.matches(lexicon, &*value.root)
                    || term.matches(lexicon, &value.lemma)
                    || term.matches(lexicon, &value.lemma_search);
                (matches, term)
            }

            Leaf::Qualified { qualifier, term } => match qualifier {
                Qualifier::Lemma => {
                    let matches = term.matches(lexicon, &value.lemma)
                        || term.matches(lexicon, &value.lemma_search)
                        || term.matches(lexicon, &value.lemma_bw);
                    (matches, term)
                }
                Qualifier::Analysis | Qualifier::Gloss => (false, term),
            },
        };

        (matches, lexicon.tf_idf(value, term))
    }
}

impl Matches<Definition> for Leaf {
    type Result = bool;

    fn matches(&self, lexicon: &Lexicon, value: &Definition) -> Self::Result {
        match self {
            Leaf::Term { term } => {
                term.matches(lexicon, &value.form)
                    || term.matches(lexicon, &value.transcription.bw)
                    || value
                        .glosses
                        .iter()
                        .any(|gloss| term.matches(lexicon, gloss))
            }
            Leaf::Qualified { qualifier, term } => match qualifier {
                Qualifier::Lemma => {
                    term.matches(lexicon, &value.form)
                        || term.matches(lexicon, &value.transcription.bw)
                        || term.matches(lexicon, &value.transcription.caphipp)
                }
                Qualifier::Analysis => term.matches(lexicon, &value.analysis),
                Qualifier::Gloss => value
                    .glosses
                    .iter()
                    .any(|gloss| term.matches(lexicon, gloss)),
            },
        }
    }
}

impl Matches<Phrase> for Leaf {
    type Result = bool;

    fn matches(&self, lexicon: &Lexicon, value: &Phrase) -> Self::Result {
        match self {
            Leaf::Term { term } => {
                term.matches(lexicon, &value.form)
                    || term.matches(lexicon, &value.transcription.bw)
                    || value
                        .glosses
                        .iter()
                        .any(|gloss| term.matches(lexicon, gloss))
            }
            Leaf::Qualified { qualifier, term } => match qualifier {
                Qualifier::Lemma => {
                    term.matches(lexicon, &value.form)
                        || term.matches(lexicon, &value.transcription.bw)
                        || term.matches(lexicon, &value.transcription.caphipp)
                }
                Qualifier::Analysis => false,
                Qualifier::Gloss => value
                    .glosses
                    .iter()
                    .any(|gloss| term.matches(lexicon, gloss)),
            },
        }
    }
}
