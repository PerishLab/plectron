use crate::{Artifact, Mapping, Origin, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub specifier: String,
    pub span: Span,
    pub source: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub dependency: usize,
    pub imported: String,
    pub local: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub exported: String,
    pub local: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syntax {
    pub dependencies: Vec<Dependency>,
    pub bindings: Vec<Binding>,
    pub exports: Vec<Export>,
    pub body: Vec<Body>,
    pub(super) edits: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Edit {
    pub span: Span,
    pub replacement: String,
}

impl Syntax {
    pub fn render(&self, source: &str) -> String {
        let mut output = String::new();
        let mut start = 0;
        for edit in &self.edits {
            output.push_str(&source[start..edit.span.start]);
            output.push_str(&edit.replacement);
            start = edit.span.end;
        }
        output.push_str(&source[start..]);
        output
    }

    pub(crate) fn emit(&self, artifact: &Artifact) -> (String, Vec<Origin>) {
        let mut output = String::new();
        let mut origins = Vec::new();
        let mut start = 0;
        for edit in &self.edits {
            append(
                &mut output,
                &mut origins,
                artifact,
                Span {
                    start,
                    end: edit.span.start,
                },
            );
            let position = output.len();
            output.push_str(&edit.replacement);
            let origin = super::locate(&artifact.origins, edit.span);
            if let Some(origin) = origin.filter(|_| position < output.len()) {
                let mut origin = origin;
                origin.mapping = Mapping::Range;
                origin.generated = Span {
                    start: position,
                    end: output.len(),
                };
                origins.push(origin);
            }
            start = edit.span.end;
        }
        append(
            &mut output,
            &mut origins,
            artifact,
            Span {
                start,
                end: artifact.text.len(),
            },
        );
        (output, origins)
    }
}

fn append(output: &mut String, origins: &mut Vec<Origin>, artifact: &Artifact, span: Span) {
    let position = output.len();
    output.push_str(&artifact.text[span.start..span.end]);
    for origin in &artifact.origins {
        let start = span.start.max(origin.generated.start);
        let end = span.end.min(origin.generated.end);
        if start < end {
            let mut projected = origin.project(Span { start, end });
            projected.generated = Span {
                start: position + start - span.start,
                end: position + end - span.start,
            };
            origins.push(projected);
        }
    }
}

pub(super) fn body(length: usize, edits: &[Edit]) -> Vec<Body> {
    let mut body = Vec::new();
    let mut start = 0;
    for edit in edits {
        if start < edit.span.start {
            body.push(Body {
                span: Span {
                    start,
                    end: edit.span.start,
                },
            });
        }
        start = edit.span.end;
    }
    if start < length {
        body.push(Body {
            span: Span { start, end: length },
        });
    }
    body
}
