use jsruntime::{Addons, Location, Precision, Provenance, Runtime, Script};
use project::{Mapping, Output, Span};
use std::ops::Range;
use std::sync::Arc;

struct Projection {
    origins: Vec<project::Origin>,
}

impl Provenance for Projection {
    fn locate(&self, span: Range<usize>) -> Option<Location> {
        let origin = project::locate(
            &self.origins,
            Span {
                start: span.start,
                end: span.end,
            },
        )?;
        Some(Location {
            resource: origin.module.path().into(),
            span: origin.source.start..origin.source.end,
            precision: match origin.mapping {
                Mapping::Exact => Precision::Exact,
                Mapping::Range => Precision::Range,
            },
        })
    }
}

pub fn script(output: Output) -> Script {
    mapped(output.javascript, output.origins)
}

pub fn mapped(text: String, origins: Vec<project::Origin>) -> Script {
    Script::new(text, Arc::new(Projection { origins }))
}

pub enum Input {
    Anonymous(String),
    Mapped(Script),
}

impl Input {
    pub fn text(&self) -> &str {
        match self {
            Self::Anonymous(text) => text,
            Self::Mapped(script) => script.source().text(),
        }
    }

    pub fn execute<A: Addons>(
        &self,
        runtime: &mut Runtime<A>,
    ) -> Result<jsengine::Value, jsengine::Failure> {
        match self {
            Self::Anonymous(text) => runtime.execute(text),
            Self::Mapped(script) => runtime.script(script),
        }
    }
}

impl From<Script> for Input {
    fn from(script: Script) -> Self {
        Self::Mapped(script)
    }
}

impl From<&str> for Input {
    fn from(text: &str) -> Self {
        Self::Anonymous(text.into())
    }
}

impl From<&String> for Input {
    fn from(text: &String) -> Self {
        Self::from(text.as_str())
    }
}
