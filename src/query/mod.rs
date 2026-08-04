use std::fmt::{Debug, Display};

use anyhow::{Result, bail, ensure};

use crate::{
    query::lexer::{Lexer, Token},
    string::{CaseFoldedNfkcNormalizedString, NonFoldedNfkcNormalizedString},
};

pub(crate) use matching::Matches;

mod lexer;
mod matching;

pub(crate) struct TermString {
    pub(crate) non_folded: NonFoldedNfkcNormalizedString,
    pub(crate) case_folded: CaseFoldedNfkcNormalizedString,
}

pub(crate) struct Term {
    pub(crate) term: TermString,
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
            term: TermString {
                non_folded: NonFoldedNfkcNormalizedString::allocated(value.chars()),
                case_folded: CaseFoldedNfkcNormalizedString::allocated(value.chars()),
            },
            anchor_start,
            anchor_end,
        }
    }
}

impl Debug for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Term")
            .field(&format!(
                "{}{}{}",
                if self.anchor_start { "^" } else { "" },
                self.term.non_folded.as_str(),
                if self.anchor_end { "$" } else { "" }
            ))
            .finish()
    }
}

#[derive(Debug)]
pub(crate) enum Qualifier {
    Analysis,
    Gloss,
    Lemma,
}

impl TryFrom<&str> for Qualifier {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "analysis" => Ok(Self::Analysis),
            "gloss" => Ok(Self::Gloss),
            "lemma" => Ok(Self::Lemma),
            other => bail!("Unknown qualifier {other:?}, expected analysis, lemma, or gloss"),
        }
    }
}

impl Display for Qualifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Qualifier::Analysis => f.write_str("analysis"),
            Qualifier::Gloss => f.write_str("gloss"),
            Qualifier::Lemma => f.write_str("lemma"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Operator {
    And,
    Or,
}

pub(crate) enum Leaf {
    Term { term: Term },
    Qualified { qualifier: Qualifier, term: Term },
}

impl Debug for Leaf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Leaf::Term { term } => write!(f, "{term:?}"),
            Leaf::Qualified { qualifier, term } => f
                .debug_tuple("Qualified")
                .field(&format!("{qualifier}:{}", term.term.non_folded.as_str()))
                .finish(),
        }
    }
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
                    term: TermString {
                        non_folded: NonFoldedNfkcNormalizedString::interned("".chars()),
                        case_folded: CaseFoldedNfkcNormalizedString::interned("".chars()),
                    },
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
