use oxc_diagnostics::Diagnostics;
use project::{Diagnostic, Source, Span};

pub(super) fn span(span: oxc_span::Span) -> Span {
    Span {
        start: span.start as usize,
        end: span.end as usize,
    }
}

pub(super) fn check(source: &Source, diagnostics: &Diagnostics) -> Result<(), Diagnostic> {
    let Some(error) = diagnostics.first() else {
        return Ok(());
    };
    let label = error
        .labels
        .iter()
        .find(|label| label.primary())
        .or_else(|| error.labels.first());
    let span = label.map_or(Span::default(), |label| Span {
        start: label.offset() as usize,
        end: label.offset() as usize + label.len() as usize,
    });
    Err(Diagnostic::new(
        source.module.clone(),
        span,
        error.message.to_string(),
    ))
}
