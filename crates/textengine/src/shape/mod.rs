mod buffer;
mod coverage;
mod filter;
mod grapheme;
mod model;
mod orientation;
mod plan;
pub(crate) use plan::Compiler;
mod position;
mod substitute;
use crate::font::Source;
use crate::{Catalog, Fault, Font, Request};
use buffer::Buffer;
pub use buffer::{Prepared, Selection};
pub use model::{Cluster, Glyph, Issue, Run, Settings, Shaped, Span};

pub trait Shaper {
    fn revision(&self) -> u64;
    fn select(
        &self,
        source: &str,
        request: &Request,
        spacing: usize,
    ) -> Result<Vec<Selection>, Fault>;
    fn prepare(&self, source: &str, request: &Request, spacing: usize) -> Result<Prepared, Fault> {
        Prepared::new(source, self.select(source, request, spacing)?)
    }
    fn shape(&self, source: &str, request: &Request, spacing: usize) -> Result<Run, Fault> {
        self.prepare(source, request, spacing)?
            .shape(0..source.len())
    }
}
pub struct Text;
impl Shaper for Text {
    fn revision(&self) -> u64 {
        1
    }
    fn select(
        &self,
        source: &str,
        request: &Request,
        spacing: usize,
    ) -> Result<Vec<Selection>, Fault> {
        static CATALOG: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
        CATALOG
            .get_or_init(Catalog::new)
            .select(source, request, spacing)
    }
}
impl Font {
    pub fn prepare(&self, source: &str, spacing: usize) -> Result<Prepared, Fault> {
        Prepared::new(
            source,
            vec![Selection {
                source: 0..source.len(),
                font: self.clone(),
                spacing,
            }],
        )
    }
    fn settings(&self, source: &str, spacing: usize) -> Settings {
        let mut settings = Settings::new(self.size, spacing);
        if self.source().is_some() && source.chars().any(|character| matches!(character as u32, 0x600..=0x6ff | 0x750..=0x77f | 0x8a0..=0x8ff)) {
            settings.script = *b"arab";
        }
        settings
    }
    pub fn shape(&self, source: &str, spacing: usize) -> Result<Run, Fault> {
        self.prepare(source, spacing)?.shape(0..source.len())
    }
}
impl Source {
    pub fn shape(&self, source: &str, settings: Settings) -> Result<Shaped, Fault> {
        if settings.size.value() < 0.0 || settings.size.value() > 4096.0 || settings.spacing > 4096
        {
            return Err(Fault("text metrics budget exhausted".into()));
        }
        let mut buffer = Buffer::new(source, |character| self.index(character))?;
        if settings.script == *b"arab" {
            buffer.forms(source);
        }
        let shaped = self.apply(buffer, settings)?;
        shaped.validate(source)?;
        Ok(shaped)
    }
    fn apply(&self, mut buffer: Buffer, settings: Settings) -> Result<Shaped, Fault> {
        if settings.script == *b"arab" {
            for feature in [*b"ccmp", *b"isol", *b"fina", *b"medi", *b"init", *b"rlig"] {
                let selected = Settings {
                    feature: Some(feature),
                    ..settings
                };
                let plan = plan::Plan::new(self, *b"GSUB", selected)?;
                buffer.substitute(&plan)?;
                buffer.issues.extend(plan.issues);
            }
        }
        let substitutions = plan::Plan::new(self, *b"GSUB", settings)?;
        buffer.substitute(&substitutions)?;
        let scale = settings.size.value() / f64::from(self.units());
        buffer.measure(|glyph| Ok(crate::Scalar::new(f64::from(self.advance(glyph)?) * scale)))?;
        let positioning = plan::Plan::new(self, *b"GPOS", settings)?;
        buffer.position(&positioning, scale)?;
        buffer.issues.extend(substitutions.issues);
        buffer.issues.extend(positioning.issues);
        buffer.finish(settings.spacing)
    }
}
