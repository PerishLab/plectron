mod cursor;
mod error;
mod format;
mod pattern;
mod run;
mod source;
mod stream;
mod validate;

pub use cursor::{Budget, Cursor, Match};
pub use error::{Error, Kind};
use pattern::Pattern;
pub use source::{Location, Source, Span};
use std::collections::BTreeSet;
pub use stream::Stream;

pub struct Grammar {
    rules: Vec<(String, Pattern)>,
    nullable: BTreeSet<String>,
}

impl Grammar {
    pub fn compile(source: &str) -> Result<Self, Error> {
        let mut grammar = Self {
            rules: format::parse(source)?,
            nullable: BTreeSet::new(),
        };
        grammar.nullable = validate::check(&grammar)?;
        Ok(grammar)
    }

    pub fn matches<'a>(
        &self,
        rule: &str,
        cursor: &mut Cursor<'a>,
    ) -> Result<Option<Match<'a>>, Error> {
        let pattern = self.rule(rule)?;
        let start = cursor.offset();
        let end = self.run(pattern, cursor, start, 0)?;
        Ok(end.map(|end| cursor.commit(start, end)))
    }

    pub fn require<'a>(&self, rule: &str, cursor: &mut Cursor<'a>) -> Result<Match<'a>, Error> {
        self.matches(rule, cursor)?
            .ok_or_else(|| Error::new(Kind::Syntax, cursor.offset(), format!("expected {rule}")))
    }

    pub fn lex<'a>(
        &self,
        rules: &[&str],
        cursor: &mut Cursor<'a>,
    ) -> Result<Option<(&str, Match<'a>)>, Error> {
        for rule in rules {
            self.rule(rule)?;
            if self.nullable.contains(*rule) {
                return Err(Error::grammar("nullable lexer rule"));
            }
        }
        let start = cursor.offset();
        let mut best = None;
        for (name, pattern) in &self.rules {
            if !rules.contains(&name.as_str()) {
                continue;
            }
            let Some(end) = self.run(pattern, cursor, start, 0)? else {
                continue;
            };
            if end == start {
                return Err(Error::grammar("nullable lexer rule"));
            }
            if best.is_none_or(|(_, prior)| end > prior) {
                best = Some((name.as_str(), end));
            }
        }
        Ok(best.map(|(name, end)| (name, cursor.commit(start, end))))
    }

    fn rule(&self, name: &str) -> Result<&Pattern, Error> {
        self.rules
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, pattern)| pattern)
            .ok_or_else(|| Error::grammar(format!("unknown rule: {name}")))
    }
}
