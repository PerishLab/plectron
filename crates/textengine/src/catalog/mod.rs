use crate::{Face, Fault, Font, Request, Selection, Shaper, Weight};
use std::sync::{Arc, RwLock};
mod fallback;
mod selection;

#[derive(Clone)]
pub struct Catalog(Arc<RwLock<State>>);
struct State {
    selector: selection::Selector,
    compiler: crate::shape::Compiler,
    revision: u64,
    closed: bool,
    sequence: u64,
    entries: Vec<Entry>,
}
#[derive(Clone)]
struct Entry {
    id: u64,
    family: String,
    weight: Weight,
    face: Face,
}
pub struct Registration {
    owner: Catalog,
    id: u64,
}
impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}
impl Catalog {
    pub fn new() -> Self {
        Self::configured(&shared::cache::Manager::default(), config::Store::default())
            .expect("fresh font selection cache")
    }
    pub fn configured(
        manager: &shared::cache::Manager,
        config: config::Store,
    ) -> Result<Self, shared::cache::Fault> {
        Ok(Self(Arc::new(RwLock::new(State {
            selector: selection::Selector::new(manager, config.clone())?,
            compiler: crate::shape::Compiler::new(manager, config)?,
            closed: false,
            revision: 1,
            sequence: 0,
            entries: Vec::new(),
        }))))
    }
    pub fn maintain(&self) {
        let mut state = self.0.write().expect("font catalog lock");
        state.selector.maintain();
        state.compiler.maintain();
    }
    pub fn register(
        &self,
        family: &str,
        weight: Weight,
        face: Face,
    ) -> Result<Registration, Fault> {
        crate::request::family::validate(family)?;
        let mut state = self.0.write().expect("font catalog lock");
        if state.closed {
            return Err(Fault("font catalog is closed".into()));
        }
        if state.entries.len() >= 256 {
            return Err(Fault("font registration budget exhausted".into()));
        }
        state.sequence = state
            .sequence
            .checked_add(1)
            .expect("font registration identity");
        let id = state.sequence;
        state.entries.push(Entry {
            id,
            family: family.to_lowercase(),
            weight,
            face,
        });
        state.selector.clear();
        state.revision = state
            .revision
            .checked_add(1)
            .expect("font catalog revision");
        Ok(Registration {
            owner: self.clone(),
            id,
        })
    }
    pub fn unregister(&self, registration: &Registration) -> Result<bool, Fault> {
        if !Arc::ptr_eq(&self.0, &registration.owner.0) {
            return Err(Fault("foreign font registration".into()));
        }
        let mut state = self.0.write().expect("font catalog lock");
        let before = state.entries.len();
        state.entries.retain(|entry| entry.id != registration.id);
        let removed = state.entries.len() != before;
        if removed {
            state.compiler.clear();
            state.selector.clear();
            state.revision = state
                .revision
                .checked_add(1)
                .expect("font catalog revision");
        }
        Ok(removed)
    }
    pub fn close(&self) {
        let mut state = self.0.write().expect("font catalog lock");
        if state.closed {
            return;
        }
        state.closed = true;
        state.selector.close();
        state.compiler.close();
        state.entries.clear();
        state.revision = state
            .revision
            .checked_add(1)
            .expect("font catalog revision");
    }
}
impl State {
    fn candidates(&self, request: &Request) -> Vec<Font> {
        let mut fonts = Vec::new();
        for family in request.families.iter() {
            let entry = self
                .entries
                .iter()
                .rev()
                .filter(|entry| entry.family == family.to_lowercase())
                .min_by_key(|entry| usize::from(entry.weight != request.weight));
            if let Some(entry) = entry {
                fonts.push(Font {
                    face: entry.face.clone(),
                    size: request.size,
                    weight: entry.weight,
                });
            }
        }
        if fonts.is_empty() {
            fonts.push(Font {
                face: if request.families.iter().any(|name| name == "toy") {
                    Face::Toy
                } else {
                    Face::Missing
                },
                size: request.size,
                weight: request.weight,
            });
        }
        fonts
    }
}
impl Shaper for Catalog {
    fn prepare(
        &self,
        source: &str,
        request: &Request,
        spacing: usize,
    ) -> Result<crate::Prepared, Fault> {
        let mut prepared = crate::Prepared::new(source, self.select(source, request, spacing)?)?;
        let mut state = self.0.write().expect("font catalog lock");
        if state.closed {
            return Err(Fault("font catalog is closed".into()));
        }
        prepared.compile(&mut state.compiler)?;
        Ok(prepared)
    }
    fn revision(&self) -> u64 {
        self.0.read().expect("font catalog lock").revision
    }
    fn select(
        &self,
        source: &str,
        request: &Request,
        spacing: usize,
    ) -> Result<Vec<Selection>, Fault> {
        request.validate()?;
        let mut state = self.0.write().expect("font catalog lock");
        if state.closed {
            return Err(Fault("font catalog is closed".into()));
        }
        state.selector.configure()?;
        let fonts = state.candidates(request);
        let result = fallback::select(source, &fonts, spacing, &mut state.selector);
        state.selector.publish();
        result
    }
}
