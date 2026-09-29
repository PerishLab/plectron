mod entry;
mod lowering;
mod report;
use crate::{Engine, Failure, Fault, Value};
use entry::Entry;
pub use report::{Function, Mode, Report};
use std::collections::BTreeMap;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct State {
    mode: Mode,
    entries: BTreeMap<usize, Entry>,
    omitted: u64,
    compiled: usize,
}

impl State {
    pub(crate) fn prune(&mut self, closures: &HashSet<usize>) {
        self.entries.retain(|slot, _| closures.contains(slot));
        self.compiled = self
            .entries
            .values()
            .filter(|entry| entry.compiled.is_some())
            .count();
    }
}

impl Engine {
    pub fn tier(&mut self, mode: Mode) {
        self.jit.mode = mode;
    }

    pub fn performance(&self) -> Report {
        Report {
            mode: self.jit.mode,
            omitted: self.jit.omitted,
            functions: self
                .jit
                .entries
                .values()
                .map(|entry| entry.report.clone())
                .collect(),
        }
    }

    pub(crate) fn accelerate(
        &mut self,
        slot: usize,
        arguments: &[Value],
    ) -> Option<Result<Value, Failure>> {
        if !self.jit.entries.contains_key(&slot) && self.jit.entries.len() >= 256 {
            self.jit.omitted = self.jit.omitted.saturating_add(1);
            return None;
        }
        let entry = self.jit.entries.entry(slot).or_default();
        entry.report.id = slot;
        entry.report.calls = entry.report.calls.saturating_add(1);
        let ready = match self.jit.mode {
            Mode::Off => false,
            Mode::Eager => true,
            Mode::Hot(count) => entry.report.calls >= count.max(1),
        };
        if ready {
            self.prepare(slot);
            if let Some(result) = self.compiled(slot, arguments) {
                return Some(result);
            }
        }
        let report = &mut self
            .jit
            .entries
            .get_mut(&slot)
            .expect("admitted entry")
            .report;
        report.interpreted = report.interpreted.saturating_add(1);
        None
    }

    fn prepare(&mut self, slot: usize) {
        let entry = self.jit.entries.get_mut(&slot).expect("admitted entry");
        if entry.compiled.is_some() || entry.report.rejection.is_some() {
            return;
        }
        if self.jit.compiled >= 64 {
            entry.report.rejection = Some("compiled function budget exceeded".into());
            return;
        }
        entry.compile(&self.closures[slot].code);
        self.jit.compiled += usize::from(entry.compiled.is_some());
    }

    fn compiled(&mut self, slot: usize, arguments: &[Value]) -> Option<Result<Value, Failure>> {
        let entry = self.jit.entries.get_mut(&slot).expect("admitted entry");
        let compiled = entry.compiled.as_ref()?;
        let lowering = &compiled.lowering;
        let mut numbers = [0.0; 16];
        if !guard(arguments, &mut numbers[..lowering.program.arity()]) {
            entry.report.guards = entry.report.guards.saturating_add(1);
            return None;
        }
        if self.budget < lowering.fuel || self.depth + lowering.depth > crate::DEPTH {
            entry.report.budgets = entry.report.budgets.saturating_add(1);
            return None;
        }
        self.budget -= lowering.fuel;
        entry.report.native = entry.report.native.saturating_add(1);
        Some(
            compiled
                .code
                .run(&numbers)
                .map(Value::Number)
                .map_err(|reason| Fault(reason).into()),
        )
    }
}

fn guard(arguments: &[Value], numbers: &mut [f64]) -> bool {
    if arguments.len() < numbers.len() {
        return false;
    }
    for (value, number) in arguments.iter().zip(numbers) {
        let Value::Number(value) = value else {
            return false;
        };
        *number = *value;
    }
    true
}
