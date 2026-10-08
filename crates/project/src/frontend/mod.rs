mod exports;
mod facts;
mod origins;
mod token;

use crate::{Diagnostic, Source, Span};
pub use facts::{Binding, Body, Dependency, Export, Syntax};
use facts::{Edit, body};
pub use origins::locate;
use token::{Kind, Token};

pub fn parse(source: &Source) -> Result<Syntax, Diagnostic> {
    let tokens = token::scan(source)?;
    Parser {
        source,
        tokens: &tokens,
        dependencies: Vec::new(),
        bindings: Vec::new(),
        exports: Vec::new(),
        edits: Vec::new(),
    }
    .parse()
}

struct Parser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    dependencies: Vec<Dependency>,
    bindings: Vec<Binding>,
    exports: Vec<Export>,
    edits: Vec<Edit>,
}

impl Parser<'_> {
    fn parse(mut self) -> Result<Syntax, Diagnostic> {
        let mut index = 0;
        while index < self.tokens.len() {
            let token = &self.tokens[index];
            if token.depth == [0; 3] && token.word("import") {
                index = self.import(index)?;
            } else if token.depth == [0; 3] && token.word("export") {
                index = self.export(index)?;
            } else {
                index += 1;
            }
        }
        self.edits.sort_by_key(|edit| edit.span.start);
        let body = body(self.source.text.len(), &self.edits);
        let Parser {
            dependencies,
            bindings,
            exports,
            edits,
            ..
        } = self;
        Ok(Syntax {
            dependencies,
            bindings,
            exports,
            body,
            edits,
        })
    }

    fn import(&mut self, start: usize) -> Result<usize, Diagnostic> {
        let next = self.token(start + 1, "incomplete import declaration")?;
        if let Kind::Text(specifier) = &next.kind {
            return self.dependency(start, start + 1, specifier.clone(), Vec::new());
        }
        if let Some(local) = next.name()
            && self.word(start + 2, "from")
        {
            let source = self.text(start + 3, "expected import source")?;
            return self.dependency(
                start,
                start + 3,
                source,
                vec![("default".into(), local.into(), next.span)],
            );
        }
        if next.symbol('{') {
            return self.named(start);
        }
        Err(self.error(next.span, "unsupported import declaration"))
    }

    fn named(&mut self, start: usize) -> Result<usize, Diagnostic> {
        let mut names = Vec::new();
        let mut index = start + 2;
        loop {
            let token = self.token(index, "unterminated import bindings")?;
            if token.symbol('}') {
                break;
            }
            let imported = token
                .name()
                .ok_or_else(|| self.error(token.span, "expected imported binding"))?;
            let mut local = imported;
            index += 1;
            if self.word(index, "as") {
                let token = self.token(index + 1, "expected local binding")?;
                local = token
                    .name()
                    .ok_or_else(|| self.error(token.span, "expected local binding"))?;
                index += 2;
            }
            names.push((imported.into(), local.into(), token.span));
            let separator = self.token(index, "unterminated import bindings")?;
            if separator.symbol(',') {
                index += 1;
            } else if !separator.symbol('}') {
                return Err(self.error(separator.span, "expected ',' or '}'"));
            }
        }
        if !self.word(index + 1, "from") {
            return Err(self.error(self.at(index + 1), "expected 'from'"));
        }
        let source = self.text(index + 2, "expected import source")?;
        self.dependency(start, index + 2, source, names)
    }

    fn dependency(
        &mut self,
        start: usize,
        source: usize,
        specifier: String,
        names: Vec<(String, String, Span)>,
    ) -> Result<usize, Diagnostic> {
        let end = self.semicolon(source + 1)?;
        let dependency = self.dependencies.len();
        let span = Span {
            start: self.tokens[start].span.start,
            end: self.tokens[end].span.end,
        };
        self.dependencies.push(Dependency {
            specifier,
            span,
            source: self.tokens[source].span,
        });
        self.bindings.extend(names.into_iter().map(|name| Binding {
            dependency,
            imported: name.0,
            local: name.1,
            span: name.2,
        }));
        self.edits.push(Edit {
            span,
            replacement: String::new(),
        });
        Ok(end + 1)
    }

    fn export(&mut self, start: usize) -> Result<usize, Diagnostic> {
        let next = self.token(start + 1, "incomplete export declaration")?;
        if next.word("default") && self.word(start + 2, "function") {
            return self.function(start);
        }
        if next.word("const") {
            return self.constant(start);
        }
        if next.symbol('{') {
            return self.exports(start);
        }
        Err(self.error(next.span, "unsupported export declaration"))
    }

    fn function(&mut self, start: usize) -> Result<usize, Diagnostic> {
        let name = self.token(start + 3, "expected default function name")?;
        let local = name
            .name()
            .ok_or_else(|| self.error(name.span, "expected default function name"))?
            .to_string();
        let span = name.span;
        self.exports.push(Export {
            exported: "default".into(),
            local: local.clone(),
            span,
        });
        self.edits.push(Edit {
            span: Span {
                start: self.tokens[start].span.start,
                end: span.end,
            },
            replacement: format!("const {local} = function"),
        });
        Ok(start + 4)
    }

    fn constant(&mut self, start: usize) -> Result<usize, Diagnostic> {
        let name = self.token(start + 2, "expected exported const name")?;
        let local = name
            .name()
            .ok_or_else(|| self.error(name.span, "expected exported const name"))?
            .to_string();
        let span = name.span;
        self.exports.push(Export {
            exported: local.clone(),
            local,
            span,
        });
        self.edits.push(Edit {
            span: Span {
                start: self.tokens[start].span.start,
                end: self.tokens[start + 1].span.start,
            },
            replacement: String::new(),
        });
        Ok(start + 3)
    }

    fn semicolon(&self, index: usize) -> Result<usize, Diagnostic> {
        let token = self.token(index, "module declaration requires ';'")?;
        if token.symbol(';') {
            Ok(index)
        } else {
            Err(self.error(token.span, "module declaration requires ';'"))
        }
    }

    fn text(&self, index: usize, message: &str) -> Result<String, Diagnostic> {
        let token = self.token(index, message)?;
        match &token.kind {
            Kind::Text(value) => Ok(value.clone()),
            _ => Err(self.error(token.span, message)),
        }
    }

    fn token(&self, index: usize, message: &str) -> Result<&Token, Diagnostic> {
        self.tokens
            .get(index)
            .ok_or_else(|| self.error(self.at(index), message))
    }

    fn word(&self, index: usize, expected: &str) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| token.word(expected))
    }

    fn at(&self, index: usize) -> Span {
        self.tokens.get(index).map_or(
            Span {
                start: self.source.text.len(),
                end: self.source.text.len(),
            },
            |token| token.span,
        )
    }

    fn error(&self, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.source.module.clone(), span, message)
    }
}
