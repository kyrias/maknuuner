use anyhow::{Result, bail, ensure};

use crate::{
    lexicon::Root,
    query::lexer::{Lexer, Token},
    string::{CaseFoldedString, NormalizedString, SearchableString},
    tf_idf,
};

mod lexer;

#[derive(Debug)]
pub(crate) struct Term {
    term: NormalizedString,
    anchor_start: bool,
    anchor_end: bool,
}

impl From<&str> for Term {
    fn from(value: &str) -> Self {
        let (value, anchor_start) = match value.strip_prefix('^') {
            Some(value) => (value, true),
            None => (value, false),
        };
        let (value, anchor_end) = match value.strip_suffix('$') {
            Some(value) => (value, true),
            None => (value, false),
        };

        Term {
            term: value.into(),
            anchor_start,
            anchor_end,
        }
    }
}

pub(crate) trait MatchTerm<T> {
    fn matches(&self, value: T) -> bool;
}

impl MatchTerm<&NormalizedString> for Term {
    fn matches(&self, value: &NormalizedString) -> bool {
        match (self.anchor_start, self.anchor_end) {
            (false, false) => value.contains(self.term.as_str()),
            (true, false) => value.starts_with(self.term.as_str()),
            (true, true) => value.as_str() == self.term.as_str(),
            (false, true) => value.ends_with(self.term.as_str()),
        }
    }
}

impl MatchTerm<&CaseFoldedString> for Term {
    fn matches(&self, value: &CaseFoldedString) -> bool {
        let value: &NormalizedString = value;
        self.matches(value)
    }
}

impl MatchTerm<&SearchableString> for Term {
    fn matches(&self, value: &SearchableString) -> bool {
        self.matches(&value.folded)
    }
}

impl MatchTerm<&Root> for Term {
    fn matches(&self, value: &Root) -> bool {
        let value: &NormalizedString = value;
        self.matches(value)
    }
}

/// Convert a query [`Term`] into a sequence of TF-IDF terms.
impl tf_idf::ToTerms for &Term {
    fn to_terms(self) -> impl Iterator<Item = tf_idf::Term> {
        self.term.to_terms()
    }
}

#[derive(Debug)]
pub(crate) enum Qualifier {
    Term,
    Analysis,
    Gloss,
}

impl TryFrom<&str> for Qualifier {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "lemma" => Ok(Self::Term),
            "analysis" => Ok(Self::Analysis),
            "gloss" => Ok(Self::Gloss),
            other => bail!("Unknown qualifier {other:?}, expected lemma, analysis, or gloss"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Operator {
    And,
    Or,
}

#[derive(Debug)]
pub(crate) enum Leaf {
    Term { term: Term },
    Qualified { qualifier: Qualifier, term: Term },
}

#[derive(Debug)]
pub(crate) enum Query {
    Leaf(Leaf),
    Operator {
        op: Operator,
        lhs: Box<Query>,
        rhs: Box<Query>,
    },
}

impl Query {
    pub(crate) fn parse(query_string: &str) -> Result<Query> {
        if query_string.trim().is_empty() {
            return Ok(Query::Leaf(Leaf::Term {
                term: Term {
                    term: "".into(),
                    anchor_start: false,
                    anchor_end: false,
                },
            }));
        }

        let mut lexer = Lexer::new(query_string);

        parse_bp(&mut lexer, 0)
    }
}

fn parse_bp(lexer: &mut Lexer<'_>, min_bp: u8) -> Result<Query> {
    let mut lhs = match lexer.next()? {
        Token::String(mut term) => {
            if term.contains('\\') {
                let mut out = String::new();
                while let Some((prefix, suffix)) = term.split_once('\\') {
                    out.push_str(prefix);
                    ensure!(suffix.chars().next().unwrap() == '"');
                    term = suffix;
                }
                out.push_str(term);
                Query::Leaf(Leaf::Term {
                    term: Term::from(out.as_str()),
                })
            } else {
                Query::Leaf(Leaf::Term {
                    term: Term::from(term),
                })
            }
        }

        Token::Qualifier(qualifier) => {
            let qualifier = qualifier.try_into()?;
            let term = match lexer.next()? {
                Token::String(term) => Term::from(term),
                Token::UnquotedTerm(term) => {
                    if term.contains('\\') {
                        bail!("Unquoted term cannot contain escape sequences: {term:?}");
                    }
                    Term::from(term)
                }
                token => bail!("Expected Literal, found {token:?}"),
            };
            Query::Leaf(Leaf::Qualified { qualifier, term })
        }

        Token::UnquotedTerm(term) => {
            if term.contains('\\') {
                bail!("Unquoted term cannot contain escape sequences: {term:?}");
            }
            Query::Leaf(Leaf::Term {
                term: Term::from(term),
            })
        }

        Token::LeftParen => {
            let lhs = parse_bp(lexer, 0)?;
            ensure!(lexer.next()? == Token::RightParen);
            lhs
        }

        token => bail!("Expected String, Qualifier, or UnquotedTerm, found {token:?}"),
    };

    loop {
        let (op, skip) = match lexer.peek()? {
            Token::Eof => return Ok(lhs),

            Token::And => (Operator::And, true),
            Token::Or => (Operator::Or, true),

            Token::String(_) => (Operator::And, false),
            Token::Qualifier(_) => (Operator::And, false),
            Token::UnquotedTerm(_) => (Operator::And, false),

            Token::LeftParen => unreachable!("Left parens are handled at the start of parse_bp"),
            Token::RightParen => return Ok(lhs),
        };

        if 1 < min_bp {
            return Ok(lhs);
        }

        if skip {
            lexer.next()?;
        }

        let rhs = parse_bp(lexer, 2)?;
        lhs = Query::Operator {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
}
