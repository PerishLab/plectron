use serde::{Deserialize, Serialize};
use std::{cell::Cell, rc::Rc};
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Shaping {
    #[serde(default)]
    pub routed: usize,
    #[serde(default)]
    pub checked: usize,
    pub preparations: usize,
    pub shapes: usize,
    pub bytes: usize,
    pub candidates: usize,
    pub materialized: usize,
}
#[derive(Clone, Default)]
pub(in crate::layout) struct Trace(Rc<Cell<Shaping>>);
impl Trace {
    pub fn route(&self, retained: bool) {
        let mut report = self.0.get();
        report.checked += 1;
        report.routed += usize::from(retained);
        self.0.set(report);
    }
    pub fn inspect(&self) -> Shaping {
        self.0.get()
    }
    pub fn prepare(&self) {
        let mut report = self.0.get();
        report.preparations += 1;
        self.0.set(report);
    }
    pub fn shape(&self, bytes: usize) {
        let mut report = self.0.get();
        report.shapes += 1;
        report.bytes += bytes;
        self.0.set(report);
    }
    pub fn line(&self) {
        let mut report = self.0.get();
        report.materialized += 1;
        self.0.set(report);
    }
    pub fn candidate(&self) {
        let mut report = self.0.get();
        report.candidates += 1;
        self.0.set(report);
    }
}
