use crate::{Artifact, Diagnostic, Kind, Source, Span};

pub trait Adapter {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Javascript;

impl Adapter for Javascript {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        if source.kind != Kind::Js {
            return Err(Diagnostic::new(
                source.module,
                Span::default(),
                "source kind has no JavaScript adapter",
            ));
        }
        Ok(Artifact::source(source.module, source.kind, source.text))
    }
}
