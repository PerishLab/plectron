use crate::{Diagnostic, Kind, Module, Provider, Request, Source, Span};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Memory {
    sources: BTreeMap<Module, String>,
}

impl Memory {
    pub fn new(sources: impl IntoIterator<Item = (Module, String)>) -> Self {
        Self {
            sources: sources.into_iter().collect(),
        }
    }

    pub fn insert(&mut self, module: Module, source: impl Into<String>) -> Option<String> {
        self.sources.insert(module, source.into())
    }
}

impl Provider for Memory {
    fn read(&self, request: Request) -> Result<Source, Diagnostic> {
        let text = self.sources.get(&request.module).cloned().ok_or_else(|| {
            Diagnostic::new(
                request.module.clone(),
                Span::default(),
                format!("module not found: {}", request.module),
            )
        })?;
        let kind = Kind::of(&request.module)
            .map_err(|fault| Diagnostic::new(request.module.clone(), Span::default(), fault.0))?;
        Ok(Source {
            module: request.module,
            kind,
            text,
        })
    }
}
