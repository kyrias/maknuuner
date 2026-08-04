use crate::{
    lexicon::{Lemma, Lexicon},
    query::{Leaf, Query},
};

pub(crate) trait Rank {
    fn rank(&self, lexicon: &Lexicon, lemma: &Lemma) -> f64;
}

impl Rank for Query {
    fn rank(&self, lexicon: &Lexicon, lemma: &Lemma) -> f64 {
        match self {
            Query::Leaf(leaf) => leaf.rank(lexicon, lemma),
            Query::Operator { op: _, lhs, rhs } => {
                lhs.rank(lexicon, lemma) + rhs.rank(lexicon, lemma)
            }
        }
    }
}

impl Rank for Leaf {
    fn rank(&self, lexicon: &crate::lexicon::Lexicon, lemma: &Lemma) -> f64 {
        let term = match self {
            Leaf::Term { term } => term,
            // We calculate the TF-IDF of qualified terms as if they were unqualified.  Arguably we
            // should probably calculate the TF-IDF separately for each qualifier.
            Leaf::Qualified { qualifier: _, term } => term,
        };

        lexicon.tf_idf(lemma, term)
    }
}
