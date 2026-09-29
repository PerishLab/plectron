use super::{Plan, storage::Storage};
use crate::font::{Lease, Problem};
use crate::shape::{Settings, Shaped, buffer::Buffer};
use crate::{Fault, Scalar};
use shared::cache::{Cost, Resource};

pub(in crate::shape) struct Program {
    owner: Lease,
    forms: Vec<Storage>,
    substitutions: Storage,
    positioning: Storage,
    resource: Resource,
}
impl Program {
    pub fn new(owner: Lease, settings: Settings) -> Result<Self, Problem> {
        let source = owner.source();
        let store = |tag, settings| {
            Plan::new(source, tag, settings).map(|plan| Storage::new(plan, source.bytes()))
        };
        let mut forms = Vec::new();
        if settings.script == *b"arab" {
            for feature in [*b"ccmp", *b"isol", *b"fina", *b"medi", *b"init", *b"rlig"] {
                forms.push(store(
                    *b"GSUB",
                    Settings {
                        feature: Some(feature),
                        ..settings
                    },
                )?);
            }
        }
        let substitutions = store(*b"GSUB", settings)?;
        let positioning = store(*b"GPOS", settings)?;
        let bytes = std::mem::size_of::<Self>()
            + forms.capacity() * std::mem::size_of::<Storage>()
            + forms.iter().map(Storage::bytes).sum::<usize>()
            + substitutions.bytes()
            + positioning.bytes();
        Ok(Self {
            owner,
            forms,
            substitutions,
            positioning,
            resource: Resource::new(bytes),
        })
    }
    pub fn cost(&self) -> Cost {
        let mut cost = Cost::new(0);
        cost.resource(self.resource.clone());
        cost.resource(self.owner.resource());
        cost
    }
    pub fn apply(&self, mut buffer: Buffer, settings: Settings) -> Result<Shaped, Fault> {
        let source = self.owner.source();
        for form in &self.forms {
            if form.admits(source.bytes(), &buffer, false)? {
                let plan = form.view(source.bytes());
                buffer.substitute(&plan)?;
            }
            buffer.issues.extend_from_slice(form.issues());
        }
        if self.substitutions.admits(source.bytes(), &buffer, false)? {
            let substitutions = self.substitutions.view(source.bytes());
            buffer.substitute(&substitutions)?;
        }
        let scale = settings.size.value() / f64::from(source.units());
        buffer.measure(|glyph| Ok(Scalar::new(f64::from(source.advance(glyph)?) * scale)))?;
        if self.positioning.admits(source.bytes(), &buffer, true)? {
            let positioning = self.positioning.view(source.bytes());
            buffer.position(&positioning, scale)?;
        }
        buffer.issues.extend_from_slice(self.substitutions.issues());
        buffer.issues.extend_from_slice(self.positioning.issues());
        buffer.finish(settings.spacing)
    }
}
