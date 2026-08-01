use anyhow::{Context, Result, ensure};

#[derive(Debug, PartialEq)]
pub(super) enum Token<'a> {
    String(&'a str),
    Qualifier(&'a str),
    And,
    Or,
    Eof,
    UnquotedTerm(&'a str),
    LeftParen,
    RightParen,
}

pub(super) struct Lexer<'a> {
    query: &'a str,
    peeked: Option<Token<'a>>,
}

impl<'a> Lexer<'a> {
    pub(super) fn new(query: &'a str) -> Lexer<'a> {
        Self {
            query,
            peeked: None,
        }
    }

    pub(super) fn next(&mut self) -> Result<Token<'a>> {
        if let Some(token) = self.peeked.take() {
            return Ok(token);
        }

        self.query = self.query.trim_start();
        if self.query.is_empty() {
            return Ok(Token::Eof);
        }

        if self.query.starts_with('"') {
            return self.string();
        }

        if self.eat_char('(').is_ok() {
            return Ok(Token::LeftParen);
        }
        if self.eat_char(')').is_ok() {
            return Ok(Token::RightParen);
        }

        if let Some(qualifier) = self.qualifier()? {
            return Ok(qualifier);
        }

        if let Some(op) = self.operator()? {
            return Ok(op);
        }

        let unquoted = self.query.split(' ').next().unwrap();
        self.query = &self.query[unquoted.len()..];
        Ok(Token::UnquotedTerm(unquoted))
    }

    pub(super) fn peek(&mut self) -> Result<&Token<'a>> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next()?);
        }

        Ok(self.peeked.as_ref().unwrap())
    }

    fn eat_char(&mut self, c: char) -> Result<()> {
        ensure!(self.query.chars().next() == Some(c));

        self.query = &self.query[c.len_utf8()..];

        Ok(())
    }

    fn string(&mut self) -> Result<Token<'a>> {
        self.eat_char('"')?;

        let mut iter = self.query.char_indices().peekable();
        loop {
            let idx = match iter.next().context("EOF parsing string")? {
                (idx, '"') => idx,
                (_, '\\') => {
                    iter.next().context("EOF parsing escape sequence")?;
                    continue;
                }
                _ => continue,
            };

            let s = &self.query[..idx];
            self.query = &self.query[s.len()..];

            self.eat_char('"')?;
            return Ok(Token::String(s));
        }
    }

    fn qualifier(&mut self) -> Result<Option<Token<'a>>> {
        let Some((prefix, _)) = self.query.split_once(':') else {
            return Ok(None);
        };

        if prefix.contains(' ') {
            return Ok(None);
        }

        self.query = &self.query[prefix.len()..];
        self.eat_char(':')?;

        Ok(Some(Token::Qualifier(prefix)))
    }

    fn operator(&mut self) -> Result<Option<Token<'a>>> {
        let seg = self
            .query
            .split(' ')
            .next()
            .expect("Already checked that query isn't empty");
        let token = if seg.eq_ignore_ascii_case("and") {
            Token::And
        } else if seg.eq_ignore_ascii_case("or") {
            Token::Or
        } else {
            return Ok(None);
        };

        self.query = &self.query[seg.len()..];

        Ok(Some(token))
    }
}
