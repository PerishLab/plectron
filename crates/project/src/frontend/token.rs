use crate::{Diagnostic, Source, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kind {
    Word(String),
    Text(String),
    Symbol(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Token {
    pub kind: Kind,
    pub span: Span,
    pub depth: [usize; 3],
}

impl Token {
    pub fn word(&self, expected: &str) -> bool {
        matches!(&self.kind, Kind::Word(value) if value == expected)
    }

    pub fn name(&self) -> Option<&str> {
        match &self.kind {
            Kind::Word(value) => Some(value),
            _ => None,
        }
    }

    pub fn symbol(&self, expected: char) -> bool {
        matches!(self.kind, Kind::Symbol(value) if value == expected)
    }
}

pub(super) fn scan(source: &Source) -> Result<Vec<Token>, Diagnostic> {
    Scanner {
        source,
        offset: 0,
        depth: [0; 3],
        tokens: Vec::new(),
    }
    .scan()
}

struct Scanner<'a> {
    source: &'a Source,
    offset: usize,
    depth: [usize; 3],
    tokens: Vec<Token>,
}

impl Scanner<'_> {
    fn scan(mut self) -> Result<Vec<Token>, Diagnostic> {
        while let Some(ch) = self.current() {
            if ch.is_whitespace() {
                self.offset += ch.len_utf8();
            } else if self.starts("//") {
                self.line();
            } else if self.starts("/*") {
                self.comment()?;
            } else if matches!(ch, '\'' | '"' | '`') {
                self.string(ch)?;
            } else if start(ch) {
                self.word();
            } else {
                self.symbol(ch)?;
            }
        }
        if self.depth != [0; 3] {
            return Err(self.error(
                Span {
                    start: self.source.text.len(),
                    end: self.source.text.len(),
                },
                "unterminated JavaScript delimiter",
            ));
        }
        Ok(self.tokens)
    }

    fn line(&mut self) {
        self.offset += 2;
        while let Some(ch) = self.current() {
            self.offset += ch.len_utf8();
            if ch == '\n' {
                break;
            }
        }
    }

    fn comment(&mut self) -> Result<(), Diagnostic> {
        let start = self.offset;
        self.offset += 2;
        while self.offset < self.source.text.len() && !self.starts("*/") {
            self.offset += self.current().expect("comment character").len_utf8();
        }
        if !self.starts("*/") {
            return Err(self.error(
                Span {
                    start,
                    end: self.source.text.len(),
                },
                "unterminated block comment",
            ));
        }
        self.offset += 2;
        Ok(())
    }

    fn string(&mut self, quote: char) -> Result<(), Diagnostic> {
        let start = self.offset;
        self.offset += quote.len_utf8();
        let content = self.offset;
        let mut escaped = false;
        while let Some(ch) = self.current() {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                let end = self.offset;
                self.offset += ch.len_utf8();
                let value = self.source.text[content..end].to_string();
                self.tokens.push(Token {
                    kind: Kind::Text(value),
                    span: Span {
                        start: content,
                        end,
                    },
                    depth: self.depth,
                });
                return Ok(());
            } else if ch == '\n' && quote != '`' {
                break;
            }
            self.offset += ch.len_utf8();
        }
        Err(self.error(
            Span {
                start,
                end: self.source.text.len(),
            },
            "unterminated string",
        ))
    }

    fn word(&mut self) {
        let start = self.offset;
        while let Some(ch) = self.current() {
            if !continues(ch) {
                break;
            }
            self.offset += ch.len_utf8();
        }
        self.tokens.push(Token {
            kind: Kind::Word(self.source.text[start..self.offset].into()),
            span: Span {
                start,
                end: self.offset,
            },
            depth: self.depth,
        });
    }

    fn symbol(&mut self, ch: char) -> Result<(), Diagnostic> {
        let start = self.offset;
        let slot = match ch {
            '(' | ')' => Some(0),
            '[' | ']' => Some(1),
            '{' | '}' => Some(2),
            _ => None,
        };
        if matches!(ch, ')' | ']' | '}')
            && let Some(slot) = slot
        {
            self.depth[slot] = self.depth[slot].checked_sub(1).ok_or_else(|| {
                self.error(
                    Span {
                        start,
                        end: start + ch.len_utf8(),
                    },
                    "unmatched JavaScript delimiter",
                )
            })?;
        }
        self.offset += ch.len_utf8();
        self.tokens.push(Token {
            kind: Kind::Symbol(ch),
            span: Span {
                start,
                end: self.offset,
            },
            depth: self.depth,
        });
        if matches!(ch, '(' | '[' | '{')
            && let Some(slot) = slot
        {
            self.depth[slot] += 1;
        }
        Ok(())
    }

    fn current(&self) -> Option<char> {
        self.source.text[self.offset..].chars().next()
    }

    fn starts(&self, value: &str) -> bool {
        self.source.text[self.offset..].starts_with(value)
    }

    fn error(&self, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.source.module.clone(), span, message)
    }
}

fn start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_alphabetic()
}

fn continues(ch: char) -> bool {
    start(ch) || ch.is_numeric()
}
