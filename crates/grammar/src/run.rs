use crate::{Cursor, Error, Grammar, Pattern};

impl Grammar {
    pub(crate) fn run(
        &self,
        pattern: &Pattern,
        cursor: &mut Cursor<'_>,
        offset: usize,
        depth: usize,
    ) -> Result<Option<usize>, Error> {
        cursor.enter(depth, offset)?;
        let text = &cursor.source().text()[offset..];
        match pattern {
            Pattern::Literal(literal) => {
                cursor.spend(literal.len().max(1), offset)?;
                Ok(text.starts_with(literal).then_some(offset + literal.len()))
            }
            Pattern::Any => Ok(text.chars().next().map(|c| offset + c.len_utf8())),
            Pattern::Class(items, negative) => {
                cursor.spend(items.len(), offset)?;
                Ok(text
                    .chars()
                    .next()
                    .filter(|c| items.iter().any(|item| item.accepts(*c)) != *negative)
                    .map(|c| offset + c.len_utf8()))
            }
            Pattern::Name(name) => self.run(self.rule(name)?, cursor, offset, depth + 1),
            Pattern::Sequence(parts) => self.sequence(parts, cursor, offset, depth + 1),
            Pattern::Choice(parts) => self.choice(parts, cursor, offset, depth + 1),
            Pattern::Repeat(inner, minimum, maximum) => {
                let mut end = offset;
                let mut count = 0;
                while maximum.is_none_or(|max| count < max) {
                    let Some(next) = self.run(inner, cursor, end, depth + 1)? else {
                        break;
                    };
                    end = next;
                    count += 1;
                }
                Ok((count >= *minimum).then_some(end))
            }
            Pattern::Look(inner, positive) => {
                let matched = self.run(inner, cursor, offset, depth + 1)?.is_some();
                Ok((matched == *positive).then_some(offset))
            }
        }
    }

    fn sequence(
        &self,
        parts: &[Pattern],
        cursor: &mut Cursor<'_>,
        mut offset: usize,
        depth: usize,
    ) -> Result<Option<usize>, Error> {
        for part in parts {
            let Some(end) = self.run(part, cursor, offset, depth)? else {
                return Ok(None);
            };
            offset = end;
        }
        Ok(Some(offset))
    }

    fn choice(
        &self,
        parts: &[Pattern],
        cursor: &mut Cursor<'_>,
        offset: usize,
        depth: usize,
    ) -> Result<Option<usize>, Error> {
        for part in parts {
            if let Some(end) = self.run(part, cursor, offset, depth)? {
                return Ok(Some(end));
            }
        }
        Ok(None)
    }
}
