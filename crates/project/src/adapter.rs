use crate::{Artifact, Diagnostic, Kind, Source, Span};

pub trait Adapter {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Unavailable;

impl Adapter for Unavailable {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        Err(Diagnostic::new(
            source.module,
            Span::default(),
            format!("source kind has no adapter: {:?}", source.kind),
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Adapters<T, P> {
    typescript: T,
    templates: P,
}

impl<T, P> Adapters<T, P> {
    pub fn new(typescript: T, templates: P) -> Self {
        Self {
            typescript,
            templates,
        }
    }
}

impl<T: Adapter, P: Adapter> Adapter for Adapters<T, P> {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        match source.kind {
            Kind::Js => Javascript.translate(source),
            Kind::Ts => self.typescript.translate(source),
            Kind::Pt => self.templates.translate(source),
        }
    }
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
