use super::{Inherited, Sheet, Specified, Style, Subject};
use crate::Fault;
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

pub struct Resolver {
    selectors: Selectors,
    sheet: Sheet,
    matching: Cache<Subject, Arc<Specified>>,
    computed: Cache<(Arc<Specified>, Inherited), Arc<Style>>,
}
impl Resolver {
    pub fn keyframes(&self, name: &str) -> Option<&super::Keyframes> {
        self.sheet.keyframes(name)
    }
    pub fn new(sheet: Sheet) -> Self {
        Self::managed(sheet, &Manager::default()).expect("fresh cache manager")
    }
    pub fn managed(sheet: Sheet, manager: &Manager) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            selectors: Selectors::new(&sheet),
            sheet,
            matching: manager.register("matching", Policy::bounded(512, 1_048_576))?,
            computed: manager.register("computed", Policy::bounded(512, 1_048_576))?,
        })
    }
    pub fn replace(&mut self, sheet: Sheet) {
        self.selectors = Selectors::new(&sheet);
        self.sheet = sheet;
        self.matching.clear();
        self.computed.clear();
        self.publish();
    }
    pub fn resolve(&mut self, subject: Subject, inherited: Inherited) -> Result<Arc<Style>, Fault> {
        let subject = self.selectors.project(subject);
        let specified = match self.matching.get(&subject) {
            Some(value) => value,
            None => {
                let specified = Arc::new(self.sheet.matching(&subject)?);
                let bytes = subject.weight().saturating_add(specified.weight());
                self.matching.insert(subject, specified.clone(), bytes);
                specified
            }
        };
        let key = (specified, inherited.clone());
        if let Some(style) = self.computed.get(&key) {
            return Ok(style);
        }
        let style = Arc::new(key.0.compute(inherited)?);
        let bytes = key.0.weight().saturating_add(std::mem::size_of::<Style>())
            + key.1.typography.font.retained()
            + style.typography.font.retained();
        self.computed.insert(key, style.clone(), bytes);
        Ok(style)
    }
    pub fn maintain(&mut self) {
        self.matching.maintain();
        self.computed.maintain();
    }
    pub fn publish(&self) {
        self.matching.publish();
        self.computed.publish();
    }
    pub fn close(&mut self) {
        self.matching.close();
        self.computed.close();
    }
}

struct Selectors {
    ids: std::collections::BTreeSet<String>,
    classes: std::collections::BTreeSet<String>,
}
impl Selectors {
    fn new(sheet: &Sheet) -> Self {
        Self {
            ids: sheet
                .rules
                .iter()
                .filter_map(|(selector, _)| selector.strip_prefix('#').map(str::to_owned))
                .collect(),
            classes: sheet
                .rules
                .iter()
                .filter_map(|(selector, _)| selector.strip_prefix('.').map(str::to_owned))
                .collect(),
        }
    }
    fn project(&self, mut subject: Subject) -> Subject {
        subject.id = subject.id.filter(|id| self.ids.contains(id));
        subject.classes = subject.classes.and_then(|classes| {
            let classes = classes
                .split_whitespace()
                .filter(|name| self.classes.contains(*name))
                .collect::<std::collections::BTreeSet<_>>();
            (!classes.is_empty()).then(|| classes.into_iter().collect::<Vec<_>>().join(" "))
        });
        subject
    }
}
