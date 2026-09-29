use super::super::{Native, Web};
use crate::{Fault, error};
use domengine::{Coverage, Layout, Node};
use graphics::shared::Frame;
use jsengine::{Engine, Value};
use std::collections::BTreeMap;

const LIMIT: usize = 64;
const MARGIN: usize = 4096;

#[derive(Clone)]
struct Entry {
    node: Node,
    callback: Value,
    margin: usize,
    current: Option<State>,
}

#[derive(Clone, PartialEq, Eq)]
struct State {
    visual: u64,
    coverage: Coverage,
}

#[derive(Default)]
pub(in crate::addons) struct Visuals {
    next: u64,
    entries: BTreeMap<u64, Entry>,
}

impl Visuals {
    pub fn empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn nodes(&self, visit: &mut dyn FnMut(Node)) {
        for entry in self.entries.values() {
            visit(entry.node);
        }
    }

    pub fn values(&self, visit: &mut dyn FnMut(&Value)) {
        for entry in self.entries.values() {
            visit(&entry.callback);
        }
    }

    pub fn prune(&mut self, dead: &[Node]) {
        self.entries.retain(|_, entry| !dead.contains(&entry.node));
    }

    pub fn release(&mut self, id: u64) {
        self.entries.remove(&id);
    }

    fn attach(&mut self, node: Node, callback: Value, margin: usize) -> Result<u64, Fault> {
        if self.entries.len() >= LIMIT {
            return Err(Fault("visual observer budget exhausted".into()));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| Fault("visual observer identifiers exhausted".into()))?;
        self.entries.insert(
            self.next,
            Entry {
                node,
                callback,
                margin,
                current: None,
            },
        );
        Ok(self.next)
    }

    fn snapshot(&self) -> Vec<(u64, Entry)> {
        self.entries
            .iter()
            .map(|(id, entry)| (*id, entry.clone()))
            .collect()
    }
}

impl Web {
    pub(crate) fn visualizing(&self) -> bool {
        !self.visuals.empty()
    }

    pub(super) fn observing(
        &mut self,
        engine: &mut Engine,
        arguments: &[Value],
    ) -> Result<Value, Fault> {
        let [host, callback, rest @ ..] = arguments else {
            return Err(Fault(
                "expected visual host, callback and optional margin".into(),
            ));
        };
        if rest.len() > 1 {
            return Err(Fault("invalid visual observer options".into()));
        }
        let node = self.node(host)?;
        engine.callable(callback)?;
        let margin = match rest.first() {
            None | Some(Value::Undefined) => 0,
            Some(Value::Number(value))
                if value.is_finite()
                    && value.fract() == 0.0
                    && (0.0..=MARGIN as f64).contains(value) =>
            {
                *value as usize
            }
            _ => return Err(Fault("invalid visual observer margin".into())),
        };
        let lease = self.visuals.attach(node, callback.clone(), margin)?;
        let dispose = self.bind(engine, Native::Unvisual(lease, node));
        let result = engine.object([("dispose".into(), dispose)].into())?;
        engine.freeze(&result)?;
        Ok(result)
    }

    pub(crate) fn visualize(
        &mut self,
        engine: &mut Engine,
        layout: &Layout,
        frame: &Frame,
    ) -> Result<usize, Fault> {
        let mut delivered = 0;
        for (id, entry) in self.visuals.snapshot() {
            if !self.document.connected(entry.node).map_err(error)? {
                continue;
            }
            let Some(coverage) = layout
                .coverage(frame, entry.node, entry.margin)
                .map_err(error)?
            else {
                continue;
            };
            let state = State {
                visual: frame.visual().revision,
                coverage,
            };
            if entry.current.as_ref() == Some(&state) {
                continue;
            }
            let value = object(engine, &state)?;
            engine
                .call(entry.callback, vec![value], self)
                .map_err(|failure| Fault(engine.diagnostic(&failure).to_string()))?;
            if let Some(current) = self.visuals.entries.get_mut(&id) {
                current.current = Some(state);
            }
            delivered += 1;
        }
        Ok(delivered)
    }
}

fn object(engine: &mut Engine, state: &State) -> Result<Value, Fault> {
    let coverage = &state.coverage;
    let value = engine.object(
        [
            ("revision".into(), Value::Number(coverage.revision as f64)),
            ("visual".into(), Value::Number(state.visual as f64)),
            ("leading".into(), Value::Number(coverage.leading as f64)),
            ("trailing".into(), Value::Number(coverage.trailing as f64)),
            ("items".into(), Value::Number(coverage.items as f64)),
            ("start".into(), Value::Number(coverage.visible.start as f64)),
            ("end".into(), Value::Number(coverage.visible.end as f64)),
            ("inset".into(), Value::Number(coverage.inset as f64)),
        ]
        .into(),
    )?;
    engine.freeze(&value)?;
    Ok(value)
}
