use super::{Export, Parser, Span, facts::Edit};
use crate::Diagnostic;

impl Parser<'_> {
    pub(super) fn exports(&mut self, start: usize) -> Result<usize, Diagnostic> {
        let first = self.exports.len();
        let mut index = start + 2;
        loop {
            let token = self.token(index, "unterminated export bindings")?;
            if token.symbol('}') {
                break;
            }
            let local = token
                .name()
                .ok_or_else(|| self.error(token.span, "expected exported binding"))?;
            let mut exported = local;
            index += 1;
            if self.word(index, "as") {
                let token = self.token(index + 1, "expected export name")?;
                exported = token
                    .name()
                    .ok_or_else(|| self.error(token.span, "expected export name"))?;
                index += 2;
            }
            self.exports.push(Export {
                exported: exported.into(),
                local: local.into(),
                span: token.span,
            });
            let separator = self.token(index, "unterminated export bindings")?;
            if separator.symbol(',') {
                index += 1;
            } else if !separator.symbol('}') {
                return Err(self.error(separator.span, "expected ',' or '}'"));
            }
        }
        if self.word(index + 1, "from") {
            return self.reexport(start, index + 2, first);
        }
        let end = self.semicolon(index + 1)?;
        self.edits.push(Edit {
            span: Span {
                start: self.tokens[start].span.start,
                end: self.tokens[end].span.end,
            },
            replacement: String::new(),
        });
        Ok(end + 1)
    }

    fn reexport(&mut self, start: usize, source: usize, first: usize) -> Result<usize, Diagnostic> {
        let specifier = self.text(source, "expected export source")?;
        let names = self.exports[first..]
            .iter_mut()
            .enumerate()
            .map(|(index, export)| {
                let mut local = format!(
                    "module_reexport_{}_{}",
                    self.tokens[start].span.start, index
                );
                while self.tokens.iter().any(|token| token.name() == Some(&local)) {
                    local.push('_');
                }
                let imported = std::mem::replace(&mut export.local, local);
                (imported, export.local.clone(), export.span)
            })
            .collect();
        self.dependency(start, source, specifier, names)
    }
}
