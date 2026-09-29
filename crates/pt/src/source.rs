use crate::compile;
use project::{Adapter, Artifact, Diagnostic, Kind, Origin, Source, Span};

#[derive(Debug, Default, Clone, Copy)]
pub struct Templates;

impl Adapter for Templates {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        match source.kind {
            Kind::Js => Ok(Artifact::source(source.module, source.kind, source.text)),
            Kind::Pt => template(source),
            Kind::Ts => Err(Diagnostic::new(
                source.module,
                Span::default(),
                "TypeScript adapter is not available",
            )),
        }
    }
}

fn template(source: Source) -> Result<Artifact, Diagnostic> {
    let length = source.text.len();
    let generated = compile(&source.text)
        .map_err(|failure| Diagnostic::new(source.module.clone(), Span::default(), failure.0))?
        .javascript;
    Ok(Artifact {
        module: source.module.clone(),
        kind: Kind::Js,
        origins: vec![Origin {
            generated: Span {
                start: 0,
                end: generated.len(),
            },
            module: source.module,
            source: Span {
                start: 0,
                end: length,
            },
        }],
        text: generated,
    })
}
