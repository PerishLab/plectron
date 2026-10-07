use crate::compile;
use project::{Adapter, Artifact, Diagnostic, Kind, Mapping, Origin, Source, Span};

#[derive(Debug, Default, Clone, Copy)]
pub struct Templates;

impl Adapter for Templates {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        if source.kind != Kind::Pt {
            return Err(Diagnostic::new(
                source.module,
                Span::default(),
                "source kind has no template adapter",
            ));
        }
        template(source)
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
            mapping: Mapping::Range,
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
