use super::Reuse;
use super::admission::{Admission, POLICY};
use super::measurement::Measurement;
use crate::Constraints;
use crate::layout::input::{Entry as Node, Key, Record};
use crate::layout::measure::Measured;
use shared::cache::{Cache, Manager};
use std::sync::Arc;

mod payloads;
mod window;
use payloads::Payloads;

struct Entry {
    geometry: Arc<Measurement>,
    attachments: Arc<[geoengine::Attachment]>,
    work: usize,
}

#[derive(Clone, Copy)]
pub(in crate::layout) enum Disposition {
    Disabled,
    Bypass,
    Screen,
    Store,
}

pub(in crate::layout) enum Lookup<'a> {
    Hit(Measured<'a>, bool),
    Miss(super::Activity),
}

pub(in crate::layout) struct Memo {
    pub sequences: crate::layout::measure::sequences::Sequences,
    pub paragraphs: super::paragraphs::Paragraphs,
    pub payloads: Payloads,
    admission: Admission,
    cache: Cache<(Arc<Key>, Constraints), Entry>,
    window: window::Window,
    pub(in crate::layout) report: Reuse,
}

impl Memo {
    pub(super) fn new(
        manager: &Manager,
        config: config::Store,
    ) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            sequences: crate::layout::measure::sequences::Sequences::new(manager, config.clone())?,
            paragraphs: super::paragraphs::Paragraphs::new(manager, config.clone())?,
            payloads: Payloads::new(manager, config.clone())?,
            admission: Admission::new(config.clone()),
            cache: manager.register("geometry", POLICY)?,
            window: window::Window::new(config),
            report: Reuse::default(),
        })
    }
    pub(super) fn configure(&mut self) -> Result<(), crate::Fault> {
        self.sequences.configure()?;
        self.paragraphs.configure()?;
        self.payloads.configure()?;
        self.window.configure()?;
        if let Some(policy) = self.admission.update()? {
            self.cache
                .configure(policy)
                .map_err(|error| crate::Fault(error.to_string()))?;
        }
        Ok(())
    }
    pub(super) fn clear(&mut self) {
        self.sequences.clear();
        self.paragraphs.clear();
        self.payloads.clear();
        self.admission.clear();
        self.window.clear();
        self.cache.clear();
    }
    pub(super) fn maintain(&mut self) {
        self.sequences.maintain();
        self.paragraphs.maintain();
        self.payloads.maintain();
        self.cache.maintain();
    }
    pub(super) fn close(&mut self) {
        self.sequences.close();
        self.paragraphs.close();
        self.payloads.close();
        self.cache.close();
    }
    pub(super) fn enabled(&self) -> bool {
        self.cache.reading()
    }
    pub(in crate::layout) fn stable(&self) -> bool {
        self.cache.reading() && self.cache.writing()
    }
    pub(super) fn enable(&mut self, enabled: bool) {
        self.sequences.enable(enabled);
        self.paragraphs.enable(enabled);
        self.payloads.enable(enabled);
        let mut policy = self.cache.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.cache.configure(policy);
    }
    pub(super) fn finish(&mut self) {
        self.sequences.publish();
        self.paragraphs.publish();
        self.payloads.publish();
        let stats = self.cache.stats();
        self.report.entries = stats.entries;
        self.report.bytes = stats.bytes;
        self.cache.publish();
        self.window.clear();
    }
    pub fn find<'a>(
        &mut self,
        element: Node<'a>,
        request: (
            Constraints,
            Option<&std::collections::BTreeSet<crate::Node>>,
            Option<&mut super::Estimates>,
            &mut crate::layout::measure::snapshot::bindings::Bindings,
        ),
        remaining: &mut usize,
    ) -> Result<Lookup<'a>, crate::Fault> {
        let (constraints, demand, mut estimates, bindings) = request;
        self.report.requests += 1;
        self.report.computed += 1;
        if !self.report.enabled {
            self.report.disabled += 1;
            return Ok(Lookup::Miss(super::Activity::Disabled));
        }
        let key = (element.key.clone(), constraints);
        if self.report.enabled
            && let Some(sample) = self.window.find(&key)
        {
            if !sample
                .geometry
                .accepts(element, demand, estimates.as_deref_mut())
            {
                self.report.dependencies += 1;
                return Ok(Lookup::Miss(super::Activity::Dependency));
            }
            if *remaining < sample.work - 1 {
                self.report.budgets += 1;
                return Ok(Lookup::Miss(super::Activity::Budget));
            }
            *remaining -= sample.work - 1;
            self.report.computed -= 1;
            self.report.hits += 1;
            self.report.windowed += 1;
            let geometry = sample.geometry;
            if sample.promote {
                self.persist(key, geometry.clone(), sample.work);
            }
            return Ok(Lookup::Hit(geometry.bind(element), true));
        }
        let Some(entry) = self.cache.lookup(&key) else {
            if self.report.enabled {
                self.report.cold += 1;
            }
            return Ok(Lookup::Miss(super::Activity::Cold));
        };
        if !self.admission.accepts(entry.work) {
            self.report.bypassed += 1;
            return Ok(Lookup::Miss(super::Activity::Bypass));
        }
        if !entry.geometry.accepts(element, demand, estimates) {
            self.report.dependencies += 1;
            return Ok(Lookup::Miss(super::Activity::Dependency));
        }
        let geometry = entry.geometry.clone();
        let attachments = entry.attachments.clone();
        let work = entry.work;
        if !self.payloads.resolve(&attachments, bindings)? {
            self.report.dependencies += 1;
            return Ok(Lookup::Miss(super::Activity::Dependency));
        }
        if *remaining < work - 1 {
            self.report.budgets += 1;
            return Ok(Lookup::Miss(super::Activity::Budget));
        }
        *remaining -= work - 1;
        self.report.computed -= 1;
        self.report.hits += 1;
        Ok(Lookup::Hit(geometry.bind(element), false))
    }
    pub fn store(
        &mut self,
        measured: &Measured<'_>,
        constraints: Constraints,
        work: usize,
    ) -> Disposition {
        if !self.cache.writing() {
            return Disposition::Disabled;
        }
        let key = (measured.element.key.clone(), constraints);
        if !self.admission.accepts(work) {
            self.report.bypassed += 1;
            if self.admission.transient() && self.window.recurring(&key) {
                self.window
                    .store(key, measured.geometry.clone(), work, false);
            }
            self.observe();
            return Disposition::Bypass;
        }
        if !measured.geometry.shaped() && !self.admission.recurring(&key, work) {
            self.report.screened += 1;
            self.report.screenwork += work;
            self.report.screenmax = self.report.screenmax.max(work);
            self.report.screenbytes += measured.element.key.weight;
            self.report.screenpeak = self.report.screenpeak.max(measured.element.key.weight);
            self.window
                .store(key, measured.geometry.clone(), work, true);
            self.observe();
            return Disposition::Screen;
        }
        self.persist(key, measured.geometry.clone(), work);
        Disposition::Store
    }
    fn persist(&mut self, key: (Arc<Key>, Constraints), geometry: Arc<Measurement>, work: usize) {
        let attachments = geometry.attachments();
        let mut cost = geometry.cost();
        cost.bytes += key.0.weight
            + std::mem::size_of::<Entry>()
            + std::mem::size_of_val(attachments.as_ref());
        let before = self.cache.stats();
        self.cache.store(
            key,
            Entry {
                geometry,
                attachments,
                work,
            },
            cost,
        );
        let after = self.cache.stats();
        self.report.evicted += (after.evicted - before.evicted) as usize;
        self.report.omitted += (after.omitted - before.omitted) as usize;
    }

    fn observe(&mut self) {
        self.report.windowpeak = self.report.windowpeak.max(self.window.len());
        self.report.windowbytes = self.report.windowbytes.max(self.window.bytes());
    }
    pub fn accepts(
        &self,
        element: &Record,
        demand: Option<&std::collections::BTreeSet<crate::Node>>,
    ) -> bool {
        !element.key.isolated || demand.is_none() || self.admission.dependencies
    }
}
