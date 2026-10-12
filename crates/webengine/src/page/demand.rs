use super::{Content, Page};
use crate::{Fault, State, error};
use domengine::{Constraints, Extent, Node, Rect};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Relevance {
    pub enabled: bool,
    pub boundaries: usize,
    pub relevant: usize,
    pub pending: usize,
    #[serde(default)]
    pub estimated: usize,
    #[serde(default)]
    pub stale: usize,
    pub overscan: usize,
}

impl Content {
    pub(super) fn relevance(&self) -> Relevance {
        let boundaries = self
            .layout
            .as_ref()
            .map(|layout| layout.boundaries())
            .unwrap_or_default();
        Relevance {
            enabled: self.deferred,
            boundaries: boundaries.len(),
            relevant: if self.deferred {
                boundaries
                    .iter()
                    .filter(|boundary| self.active.contains(&boundary.node))
                    .count()
            } else {
                boundaries.len()
            },
            pending: boundaries
                .iter()
                .filter(|boundary| boundary.pending)
                .count(),
            estimated: boundaries
                .iter()
                .filter(|boundary| boundary.quality == domengine::Quality::Estimated)
                .count(),
            stale: boundaries
                .iter()
                .filter(|boundary| boundary.quality == domengine::Quality::Stale)
                .count(),
            overscan: self.overscan,
        }
    }

    pub(crate) fn viewport(&mut self, visual: graphics::shared::Visual) -> Result<bool, Fault> {
        self.check()?;
        self.timing.scene = 0;
        self.begin();
        if visual.extent != self.extent {
            return Err(Fault(
                "layout demand viewport extent differs from content".into(),
            ));
        }
        self.visual = visual;
        let changed = self.settle()?;
        self.converge(changed)
    }
    pub(super) fn settle(&mut self) -> Result<bool, Fault> {
        let mut changed = false;
        if !self.deferred {
            return Ok(changed);
        }
        for _ in 0..=32 {
            let (Some(scene), Some(layout)) = (&self.scene, &self.layout) else {
                return Ok(changed);
            };
            if layout.boundaries().is_empty() {
                return Ok(changed);
            }
            let mut visual = self.visual.clone();
            visual.offsets.retain(|id, offset| {
                if let Some(range) = scene.range(*id, visual.extent) {
                    *offset = (*offset).min(range);
                    true
                } else {
                    false
                }
            });
            let frame = graphics::shared::Frame::new(scene.clone(), visual).map_err(error)?;
            self.computation.checks += 1;
            if layout.ready(&frame) {
                return Ok(changed);
            }
            let mut demand = layout.demand(&frame, self.overscan);
            let document = &self
                .runtime
                .as_mut()
                .expect("loaded runtime")
                .addons()
                .document;
            let mut focus = document.focused();
            while let Some(node) = focus {
                demand.insert(node);
                focus = document.entry(node).map_err(error)?.parent;
            }
            if demand == self.active {
                return Ok(changed);
            }
            self.active = demand;
            self.computation.changes += 1;
            self.pipeline.demand(Some(self.active.clone()));
            self.dirty = true;
            changed |= self.build()?;
        }
        Err(Fault(
            "layout demand did not converge within scene depth".into(),
        ))
    }
    pub(crate) fn geometry(&mut self, id: &str) -> Result<Option<Rect>, Fault> {
        self.check()?;
        self.computation.queries += 1;
        let document = &self
            .runtime
            .as_mut()
            .expect("loaded runtime")
            .addons()
            .document;
        let Some(node) = document.find(id) else {
            return Ok(None);
        };
        let mut demand: BTreeSet<Node> = self.active.clone();
        let mut current = Some(node);
        while let Some(node) = current {
            demand.insert(node);
            current = document.entry(node).map_err(error)?.parent;
        }
        self.pipeline.demand(Some(demand));
        let result = self
            .capture
            .update(document, |input| {
                self.pipeline
                    .build(input, Constraints::viewport(self.extent))
            })
            .map(|layout| layout.geometry(node))
            .map_err(error);
        if result.is_ok() {
            self.computation
                .measure(&self.capture.inspect(), &self.pipeline.inspect());
        }
        self.pipeline
            .demand(self.deferred.then(|| self.active.clone()));
        result
    }
}
impl Page {
    pub(super) fn viewport(&mut self) -> Result<(), Fault> {
        if self.content.viewport(self.display.visual.clone())? {
            self.publish()?;
        }
        Ok(())
    }
    pub fn geometry(&mut self, id: &str) -> Result<Option<Rect>, Fault> {
        self.content.geometry(id)
    }
}

impl Content {
    pub(crate) fn resize(&mut self, extent: Extent) -> Result<bool, Fault> {
        extent.area().map_err(error)?;
        if matches!(self.state, State::Closed | State::Failed) {
            return Err(Fault("content cannot change viewport".into()));
        }
        if self.extent == extent {
            return Ok(false);
        }
        self.extent = extent;
        self.visual.extent = extent;
        self.dirty = true;
        if self.state == State::Ready {
            self.render()?;
        }
        Ok(true)
    }
}
impl Page {
    pub fn resize(&mut self, extent: Extent) -> Result<(), Fault> {
        if self.content.resize(extent)? {
            self.publish()?;
        }
        Ok(())
    }
}
