use crate::pattern::Item;
use crate::{Cursor, Error, Kind, Pattern};

mod class;

pub(crate) fn parse(source: &str) -> Result<Vec<(String, Pattern)>, Error> {
    let mut parser = Parser {
        cursor: Cursor::new(source)?,
        depth: 0,
        nodes: 0,
    };
    let mut rules = Vec::new();
    parser.space()?;
    while !parser.cursor.rest().is_empty() {
        let name = parser.name()?;
        parser.expect('=')?;
        let pattern = parser.choice()?;
        parser.expect(';')?;
        rules.push((name, pattern));
        if rules.len() > 256 {
            return Err(parser.limit("too many rules"));
        }
    }
    if rules.is_empty() {
        return Err(parser.fault("empty grammar"));
    }
    Ok(rules)
}

struct Parser<'a> {
    cursor: Cursor<'a>,
    depth: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn fault(&self, note: &str) -> Error {
        Error::new(Kind::Grammar, self.cursor.offset(), note)
    }

    fn limit(&self, note: &str) -> Error {
        Error::new(Kind::Budget, self.cursor.offset(), note)
    }

    fn charge(&mut self) -> Result<(), Error> {
        self.nodes += 1;
        if self.nodes > 4096 {
            return Err(self.limit("grammar structure budget exhausted"));
        }
        Ok(())
    }

    fn peek(&self) -> Option<char> {
        self.cursor.rest().chars().next()
    }

    fn space(&mut self) -> Result<(), Error> {
        while self.peek().is_some_and(char::is_whitespace) {
            self.cursor.take()?;
        }
        Ok(())
    }

    fn eat(&mut self, value: char) -> Result<bool, Error> {
        if self.peek() != Some(value) {
            return Ok(false);
        }
        self.cursor.take()?;
        self.space()?;
        Ok(true)
    }

    fn expect(&mut self, value: char) -> Result<(), Error> {
        if self.eat(value)? {
            Ok(())
        } else {
            Err(self.fault(&format!("expected {value}")))
        }
    }

    fn name(&mut self) -> Result<String, Error> {
        let mut name = String::new();
        if !self
            .peek()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return Err(self.fault("expected rule name"));
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            name.push(self.cursor.take()?.unwrap());
        }
        self.space()?;
        Ok(name)
    }

    fn choice(&mut self) -> Result<Pattern, Error> {
        self.charge()?;
        let mut choices = vec![self.sequence()?];
        while self.eat('|')? {
            choices.push(self.sequence()?);
        }
        Ok(Pattern::Choice(choices))
    }

    fn sequence(&mut self) -> Result<Pattern, Error> {
        self.charge()?;
        let mut parts = Vec::new();
        while !matches!(self.peek(), None | Some(';' | ')' | '|')) {
            parts.push(self.pattern()?);
        }
        Ok(Pattern::Sequence(parts))
    }

    fn pattern(&mut self) -> Result<Pattern, Error> {
        self.depth += 1;
        self.charge()?;
        if self.depth > 64 {
            return Err(self.limit("grammar depth exhausted"));
        }
        let mut pattern = self.atom()?;
        if self.eat('*')? {
            pattern = Pattern::Repeat(Box::new(pattern), 0, None);
        } else if self.eat('+')? {
            pattern = Pattern::Repeat(Box::new(pattern), 1, None);
        } else if self.eat('?')? {
            pattern = Pattern::Repeat(Box::new(pattern), 0, Some(1));
        }
        self.depth -= 1;
        Ok(pattern)
    }

    fn atom(&mut self) -> Result<Pattern, Error> {
        match self.peek() {
            Some('\'' | '"') => self.literal(),
            Some('[') => self.class(),
            Some('.') => {
                self.expect('.')?;
                Ok(Pattern::Any)
            }
            Some('(') => {
                self.expect('(')?;
                let pattern = self.choice()?;
                self.expect(')')?;
                Ok(pattern)
            }
            Some('!' | '&') => {
                let positive = self.cursor.take()? == Some('&');
                self.space()?;
                Ok(Pattern::Look(Box::new(self.pattern()?), positive))
            }
            _ => self.name().map(Pattern::Name),
        }
    }

    fn literal(&mut self) -> Result<Pattern, Error> {
        let quote = self.cursor.take()?.unwrap();
        let mut literal = String::new();
        loop {
            let value = self
                .cursor
                .take()?
                .ok_or_else(|| self.fault("unclosed literal"))?;
            if value == quote {
                break;
            }
            literal.push(if value == '\\' { self.escape()? } else { value });
        }
        self.space()?;
        Ok(Pattern::Literal(literal))
    }

    fn escape(&mut self) -> Result<char, Error> {
        match self.cursor.take()? {
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some(value @ ('\\' | '\'' | '"' | '[' | ']' | '-' | '^')) => Ok(value),
            _ => Err(self.fault("invalid grammar escape")),
        }
    }
}
