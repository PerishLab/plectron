mod resource;
use crate::Rejection;
use grammar::{Cursor, Grammar, Location, Stream};

const LIMIT: usize = 16_384;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Kind {
    Name(String),
    Number(f64),
    Text(String),
    Mark(String),
    End,
}

#[derive(Debug, Clone)]
pub(super) struct Token {
    pub kind: Kind,
    pub location: Location,
    pub span: std::ops::Range<usize>,
}

pub(super) fn tokens(source: &str) -> Result<Stream<Token>, Rejection> {
    if source.len() > 65_536 {
        return Err(crate::Fault("script exceeds 64 KiB budget".into()).into());
    }
    let grammar = resource::js()?;
    let mut scanner = Scanner {
        cursor: Cursor::new(source).map_err(fault)?,
        grammar,
    };
    let mut tokens = Vec::new();
    loop {
        scanner.space()?;
        let location = scanner.cursor.location();
        let start = scanner.cursor.offset();
        let kind = scanner.next()?;
        let end = kind == Kind::End;
        let token = Token {
            kind,
            location,
            span: start..scanner.cursor.offset(),
        };
        if tokens.len() >= LIMIT {
            return Err(crate::Fault("script exceeds token budget".into()).into());
        }
        if end {
            return Ok(Stream::new(tokens, token));
        }
        tokens.push(token);
    }
}

struct Scanner<'a> {
    cursor: Cursor<'a>,
    grammar: &'static Grammar,
}

impl Scanner<'_> {
    fn fault(&self, note: &str) -> Rejection {
        Rejection::syntax(format!(
            "syntax at character {}: {note}",
            self.cursor.location().character
        ))
        .at(self.cursor.offset()..self.cursor.offset())
    }

    fn space(&mut self) -> Result<(), Rejection> {
        loop {
            if self.cursor.rest().starts_with("/*") {
                self.grammar
                    .matches("comment", &mut self.cursor)
                    .map_err(fault)?
                    .ok_or_else(|| self.fault("unterminated comment"))?;
            } else if self
                .grammar
                .lex(&["space", "line"], &mut self.cursor)
                .map_err(fault)?
                .is_none()
            {
                return Ok(());
            }
        }
    }

    fn next(&mut self) -> Result<Kind, Rejection> {
        if self.cursor.rest().is_empty() {
            return Ok(Kind::End);
        }
        if let Some(quote) = self
            .grammar
            .matches("quote", &mut self.cursor)
            .map_err(fault)?
        {
            return self.string(quote.text.chars().next().unwrap());
        }
        let (rule, token) = self
            .grammar
            .lex(&["name", "number", "mark"], &mut self.cursor)
            .map_err(fault)?
            .ok_or_else(|| self.fault("unsupported token"))?;
        match rule {
            "name" => Ok(Kind::Name(token.text.into())),
            "number" => self.number(token.text),
            _ => Ok(Kind::Mark(token.text.into())),
        }
    }

    fn number(&self, text: &str) -> Result<Kind, Rejection> {
        if text.len() > 1 && text.starts_with('0') && !text.starts_with("0.") {
            return Err(self.fault("leading-zero numbers are unsupported"));
        }
        text.parse()
            .map(Kind::Number)
            .map_err(|_| self.fault("invalid number"))
    }

    fn string(&mut self, quote: char) -> Result<Kind, Rejection> {
        let mut text = String::new();
        while let Some(value) = self.cursor.take().map_err(fault)? {
            match value {
                value if value == quote => return Ok(Kind::Text(text)),
                '\\' => text.push(self.escape()?),
                '\n' | '\r' => return Err(self.fault("newline in string")),
                value => text.push(value),
            }
        }
        Err(self.fault("unterminated string"))
    }

    fn escape(&mut self) -> Result<char, Rejection> {
        let value = self
            .cursor
            .take()
            .map_err(fault)?
            .ok_or_else(|| self.fault("unterminated escape"))?;
        match value {
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            'u' => self.unicode(),
            '\\' | '\'' | '"' => Ok(value),
            _ => Err(self.fault("unsupported string escape")),
        }
    }

    fn unicode(&mut self) -> Result<char, Rejection> {
        let token = self
            .grammar
            .matches("unicode", &mut self.cursor)
            .map_err(fault)?
            .ok_or_else(|| self.fault("invalid Unicode escape"))?;
        let digits = token.text.trim_matches(['{', '}']);
        u32::from_str_radix(digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| self.fault("escape must represent a Unicode scalar value"))
    }
}

fn fault(error: impl std::fmt::Display) -> Rejection {
    crate::Fault(error.to_string()).into()
}
