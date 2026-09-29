use super::{Error, Item, Parser, Pattern};

impl Parser<'_> {
    pub(super) fn class(&mut self) -> Result<Pattern, Error> {
        self.cursor.take()?;
        let negative = self.peek() == Some('^');
        if negative {
            self.cursor.take()?;
        }
        let mut items = Vec::new();
        while self.peek() != Some(']') {
            items.push(self.item()?);
        }
        if items.is_empty() {
            return Err(self.fault("empty character class"));
        }
        self.expect(']')?;
        Ok(Pattern::Class(items, negative))
    }

    fn item(&mut self) -> Result<Item, Error> {
        if self.cursor.rest().starts_with("\\s") {
            self.cursor.advance(2)?;
            return Ok(Item::Space);
        }
        let start = self.character()?;
        if self.peek() != Some('-') || self.cursor.rest().starts_with("-]") {
            return Ok(Item::Range(start, start));
        }
        self.cursor.take()?;
        let end = self.character()?;
        if start > end {
            return Err(self.fault("reversed character range"));
        }
        Ok(Item::Range(start, end))
    }

    fn character(&mut self) -> Result<char, Error> {
        match self.cursor.take()? {
            Some('\\') => self.escape(),
            Some(']') | None => Err(self.fault("unclosed character class")),
            Some(value) => Ok(value),
        }
    }
}
