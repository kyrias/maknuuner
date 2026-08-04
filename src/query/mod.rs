use anyhow::{Result, bail, ensure};

use crate::{
    query::lexer::{Lexer, Token},
    string::{
        CaseFoldedNfkcNormalizedString, NfkcNormalizedString, NonFoldedNfkcNormalizedString,
        SearchableString,
    },
};

mod lexer;

#[derive(Debug)]
pub(crate) struct TermString {
    pub(crate) non_folded: NonFoldedNfkcNormalizedString,
    pub(crate) case_folded: CaseFoldedNfkcNormalizedString,
}

#[derive(Debug)]
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

pub(crate) trait MatchTerm<T>
where
    T: ?Sized,
{
    fn matches(&self, value: &T) -> bool;
}

impl MatchTerm<NonFoldedNfkcNormalizedString> for Term {
    fn matches(&self, string: &NonFoldedNfkcNormalizedString) -> bool {
        let string = string.as_str();
        match (self.anchor_start, self.anchor_end) {
            (false, false) => string.contains(self.term.non_folded.as_str()),
            (true, false) => string.starts_with(self.term.non_folded.as_str()),
            (true, true) => string == self.term.non_folded.as_str(),
            (false, true) => string.ends_with(self.term.non_folded.as_str()),
        }
    }
}

impl MatchTerm<CaseFoldedNfkcNormalizedString> for Term {
    fn matches(&self, string: &CaseFoldedNfkcNormalizedString) -> bool {
        let string = string.as_str();
        match (self.anchor_start, self.anchor_end) {
            (false, false) => string.contains(self.term.case_folded.as_str()),
            (true, false) => string.starts_with(self.term.case_folded.as_str()),
            (true, true) => string == self.term.case_folded.as_str(),
            (false, true) => string.ends_with(self.term.case_folded.as_str()),
        }
    }
}

impl MatchTerm<SearchableString> for Term {
    fn matches(&self, value: &SearchableString) -> bool {
        match &value.searchable {
            NfkcNormalizedString::NonFolded(inner) => self.matches(inner),
            NfkcNormalizedString::CaseFolded(inner) => self.matches(inner),
        }
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
