use crate::Fault;
use grammar::{Cursor, Grammar};
use std::collections::BTreeMap;

pub enum Token {
    Open {
        name: String,
        attributes: BTreeMap<String, String>,
    },
    Close(String),
    Text(String),
}

pub struct Scanner<'a> {
    cursor: Cursor<'a>,
    grammar: &'static Grammar,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Result<Self, Fault> {
        let grammar = crate::resource::html()?;
        Ok(Self {
            cursor: Cursor::new(source).map_err(fault)?,
            grammar,
        })
    }

    pub fn next(&mut self) -> Result<Option<Token>, Fault> {
        loop {
            if self.cursor.rest().is_empty() {
                return Ok(None);
            }
            if self.cursor.rest().starts_with("<!--") {
                self.require("comment", "unclosed HTML comment")?;
            } else if !self.eat("doctype")? {
                break;
            }
        }
        if let Some(text) = self
            .grammar
            .matches("text", &mut self.cursor)
            .map_err(fault)?
        {
            return Ok(Some(Token::Text(
                entities(text.text).map_err(|error| self.context(error))?,
            )));
        }
        self.require("open", "expected HTML tag")?;
        if self.eat("closing")? {
            let name = self
                .require("tail", "expected closing name")?
                .trim()
                .to_ascii_lowercase();
            self.require("end", "unclosed tag")?;
            return Ok(Some(Token::Close(name)));
        }
        self.open().map(Some)
    }

    fn open(&mut self) -> Result<Token, Fault> {
        let name = self
            .require("name", "expected HTML name")?
            .to_ascii_lowercase();
        let mut attributes = BTreeMap::new();
        loop {
            self.eat("space")?;
            if self.eat("end")? {
                break;
            }
            let key = self
                .require("name", "expected HTML name")?
                .to_ascii_lowercase();
            self.eat("space")?;
            self.require("assign", "attribute requires a value")?;
            let quoted = self.require("quoted", "attribute requires closed quotes")?;
            let value =
                entities(&quoted[1..quoted.len() - 1]).map_err(|error| self.context(error))?;
            if attributes.insert(key, value).is_some() {
                return Err(self.context(Fault("duplicate HTML attribute".into())));
            }
        }
        Ok(Token::Open { name, attributes })
    }

    fn eat(&mut self, rule: &str) -> Result<bool, Fault> {
        Ok(self
            .grammar
            .matches(rule, &mut self.cursor)
            .map_err(fault)?
            .is_some())
    }

    fn require(&mut self, rule: &str, note: &str) -> Result<&'a str, Fault> {
        self.grammar
            .matches(rule, &mut self.cursor)
            .map_err(fault)?
            .map(|matched| matched.text)
            .ok_or_else(|| self.context(Fault(note.into())))
    }

    pub fn context(&self, error: Fault) -> Fault {
        Fault(format!("HTML at byte {}: {error}", self.cursor.offset()))
    }
}

fn fault(error: impl std::fmt::Display) -> Fault {
    Fault(error.to_string())
}

fn entities(source: &str) -> Result<String, Fault> {
    let mut rest = source;
    let mut output = String::new();
    while let Some((prefix, tail)) = rest.split_once('&') {
        output.push_str(prefix);
        let (name, suffix) = tail
            .split_once(';')
            .ok_or_else(|| Fault("unclosed HTML entity".into()))?;
        output.push(match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => return Err(Fault(format!("unsupported entity: {name}"))),
        });
        rest = suffix;
    }
    output.push_str(rest);
    Ok(output)
}
