use jsengine::{Engine, Failure, Fault, Source};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, Weak};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub resource: String,
    pub span: Range<usize>,
    pub precision: Precision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Exact,
    Range,
}

pub trait Provenance: Send + Sync {
    fn locate(&self, span: Range<usize>) -> Option<Location>;
}

#[derive(Clone)]
pub struct Script {
    source: Source,
    provenance: Arc<dyn Provenance>,
}

impl Script {
    pub fn new(text: impl Into<Arc<str>>, provenance: Arc<dyn Provenance>) -> Self {
        Self {
            source: Source::new(text),
            provenance,
        }
    }

    pub fn source(&self) -> &Source {
        &self.source
    }
}

#[derive(Clone, Default)]
pub struct Diagnostics(Weak<Mutex<Registry>>);

impl Diagnostics {
    pub fn location(&self, failure: &Failure) -> Option<Location> {
        let site = failure.site()?;
        let sources = self.0.upgrade()?;
        let provenance = sources
            .lock()
            .expect("source diagnostics lock")
            .provenance(site.source.identity())?;
        provenance.locate(site.span.clone())
    }

    pub fn diagnostic(&self, engine: &Engine, failure: &Failure) -> Fault {
        let diagnostic = engine.diagnostic(failure);
        match self.location(failure) {
            Some(location) => {
                let precision = match location.precision {
                    Precision::Exact => "",
                    Precision::Range => " (range)",
                };
                Fault(format!(
                    "{}:{}..{}{}: {}",
                    location.resource,
                    location.span.start,
                    location.span.end,
                    precision,
                    diagnostic
                ))
            }
            None => diagnostic,
        }
    }
}

#[derive(Default)]
pub(crate) struct Sources(Arc<Mutex<Registry>>);

impl Sources {
    pub fn diagnostics(&self) -> Diagnostics {
        Diagnostics(Arc::downgrade(&self.0))
    }

    pub fn insert(&mut self, script: &Script) {
        self.prune();
        self.0
            .lock()
            .expect("source diagnostics lock")
            .insert(script);
    }

    pub fn prune(&mut self) {
        let retired = self.0.lock().expect("source diagnostics lock").prune();
        drop(retired);
    }
}

#[derive(Default)]
struct Registry(BTreeMap<u64, Entry>);

struct Entry {
    source: jsengine::Weak,
    provenance: Arc<dyn Provenance>,
}

impl Registry {
    pub fn insert(&mut self, script: &Script) {
        self.0
            .entry(script.source.identity())
            .or_insert_with(|| Entry {
                source: script.source.downgrade(),
                provenance: script.provenance.clone(),
            });
    }

    pub fn provenance(&self, identity: u64) -> Option<Arc<dyn Provenance>> {
        Some(self.0.get(&identity)?.provenance.clone())
    }

    pub fn prune(&mut self) -> Vec<Entry> {
        let expired: Vec<_> = self
            .0
            .iter()
            .filter(|(_, entry)| entry.source.upgrade().is_none())
            .map(|(identity, _)| *identity)
            .collect();
        expired
            .into_iter()
            .filter_map(|identity| self.0.remove(&identity))
            .collect()
    }
}
