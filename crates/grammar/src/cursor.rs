use crate::{Error, Kind, Location, Source, Span};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub work: usize,
    pub depth: usize,
    pub bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            work: 4_000_000,
            depth: 64,
            bytes: 65_536,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match<'a> {
    pub span: Span,
    pub text: &'a str,
}

pub struct Cursor<'a> {
    source: Source<'a>,
    offset: usize,
    location: Location,
    budget: Budget,
}

impl<'a> Cursor<'a> {
    pub fn new(text: &'a str) -> Result<Self, Error> {
        Self::bounded(text, Budget::default())
    }

    pub fn bounded(text: &'a str, budget: Budget) -> Result<Self, Error> {
        if text.len() > budget.bytes {
            return Err(Error::new(Kind::Budget, 0, "source size exhausted"));
        }
        Ok(Self {
            source: Source::new(text),
            offset: 0,
            location: Location::default(),
            budget,
        })
    }

    pub fn source(&self) -> Source<'a> {
        self.source
    }
    pub fn location(&self) -> Location {
        self.location
    }

    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn remaining(&self) -> usize {
        self.budget.work
    }
    pub fn rest(&self) -> &'a str {
        &self.source.text()[self.offset..]
    }

    pub fn advance(&mut self, bytes: usize) -> Result<Match<'a>, Error> {
        let start = self.offset;
        let end = start
            .checked_add(bytes)
            .filter(|end| self.source.text().is_char_boundary(*end));
        let end = end.ok_or_else(|| Error::new(Kind::Syntax, start, "invalid source boundary"))?;
        self.spend(bytes.max(1), start)?;
        Ok(self.commit(start, end))
    }

    pub fn take(&mut self) -> Result<Option<char>, Error> {
        let next = self.rest().chars().next();
        if let Some(value) = next {
            self.advance(value.len_utf8())?;
        }
        Ok(next)
    }

    pub(crate) fn spend(&mut self, amount: usize, offset: usize) -> Result<(), Error> {
        let remaining = self.budget.work.checked_sub(amount);
        self.budget.work = remaining.unwrap_or(0);
        remaining
            .map(|_| ())
            .ok_or_else(|| Error::new(Kind::Budget, offset, "matching work exhausted"))
    }

    pub(crate) fn enter(&mut self, depth: usize, offset: usize) -> Result<(), Error> {
        self.spend(1, offset)?;
        if depth > self.budget.depth {
            return Err(Error::new(Kind::Budget, offset, "matching depth exhausted"));
        }
        Ok(())
    }

    pub(crate) fn commit(&mut self, start: usize, end: usize) -> Match<'a> {
        self.location.advance(&self.source.text()[start..end]);
        self.offset = end;
        Match {
            span: Span { start, end },
            text: &self.source.text()[start..end],
        }
    }
}
