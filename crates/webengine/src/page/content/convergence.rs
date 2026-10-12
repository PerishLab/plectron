use super::Content;
use crate::page::{operation, timing};
use crate::{Fault, State, error};
use domengine::{Constraints, Shaper};
use graphics::shared::Scene;
use pipeline::Graph;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Convergence {
    Adaptive,
    Eager,
    Deferred,
}

pub(crate) struct Candidate {
    pub extent: domengine::Extent,
    pub layout: Arc<domengine::Layout>,
    pub scene: Scene,
    pub scrolls: Vec<crate::display::Policy>,
    pub watching: Vec<domengine::Node>,
    pub replay: Vec<domengine::Node>,
}

impl Content {
    pub(in crate::page) fn begin(&mut self) {
        self.rounds = 0;
    }
    fn deferring(&self) -> bool {
        self.convergence == Convergence::Deferred
            || (self.convergence == Convergence::Adaptive && self.rounds >= 2)
    }
    pub(in crate::page) fn candidate(&mut self) -> Option<Candidate> {
        if !self.candidate {
            return None;
        }
        let runtime = self.runtime.as_mut()?;
        Some(Candidate {
            extent: self.extent,
            layout: self.layout.clone()?,
            scene: self.scene.clone()?,
            scrolls: runtime.addons().scrolls.policies(),
            watching: runtime.addons().watching(),
            replay: runtime.addons().scrolls.replay(),
        })
    }
    pub(in crate::page) fn build(&mut self) -> Result<bool, Fault> {
        let deferring = self.deferring();
        let routing = self
            .config
            .get("scene.routing")
            .map(bool::deserialize)
            .transpose()
            .map_err(error)?
            .unwrap_or(true);
        let checks = self
            .config
            .get("scene.checks")
            .map(usize::deserialize)
            .transpose()
            .map_err(error)?
            .unwrap_or(graphics::shared::CHECKS);
        if matches!(self.state, State::Closed | State::Failed) {
            return Err(Fault("content is not renderable".into()));
        }
        let document = &mut self
            .runtime
            .as_mut()
            .ok_or_else(|| Fault("document is not loaded".into()))?
            .addons()
            .document;
        self.pictures
            .as_mut()
            .expect("loaded picture scope")
            .sync(document, &mut self.resources)?;
        let epoch = self.fonts.revision();
        if !self.dirty
            && self.epoch == epoch
            && self
                .layout
                .as_ref()
                .is_some_and(|layout| layout.revision() == document.revision())
        {
            return Ok(false);
        }
        if deferring {
            if !self.candidate {
                self.basis = self.scene.clone();
            }
            let layout = self
                .capture
                .update(document, |input| {
                    self.pipeline
                        .build(input, Constraints::viewport(self.extent))
                })
                .map_err(error)?;
            self.computation
                .measure(&self.capture.inspect(), &self.pipeline.inspect());
            let start = std::time::Instant::now();
            let scene = Scene::project(layout.extent(), layout.revision(), layout.spatial())
                .map_err(error)?;
            let projection = timing::elapsed(start);
            self.computation.projections = self.computation.projections.saturating_add(1);
            self.computation.projection = self.computation.projection.saturating_add(projection);
            self.timing.scene = self.timing.scene.saturating_add(projection);
            self.scene = Some(scene);
            self.candidate = true;
            self.effects = None;
            self.layout = Some(Arc::new(layout));
            self.epoch = epoch;
            self.dirty = false;
            self.rounds = self.rounds.saturating_add(1);
            return Ok(true);
        }
        let previous = self.timing.scene;
        let (layout, scene) = self
            .capture
            .update(document, |input| {
                operation::publication(
                    &mut self.pipeline,
                    &mut self.painter,
                    &mut self.timing.scene,
                    (self.scene.as_ref().filter(|_| routing), checks),
                )
                .run((input, Constraints::viewport(self.extent)))
            })
            .map_err(error)?;
        self.computation
            .measure(&self.capture.inspect(), &self.pipeline.inspect());
        self.computation.paints += 1;
        self.computation.finalizations = self.computation.finalizations.saturating_add(1);
        self.computation.paint += self.painter.inspect().elapsed;
        self.computation.scene += self.timing.scene - previous;
        self.epoch = epoch;
        let tracks = layout.tracks(&self.capture.plans());
        if let Some(animation) = &self.animation {
            animation.replace(scene.generation(), tracks)?;
        } else {
            self.animator.replace(scene.generation(), tracks);
        }
        self.scene = Some(scene);
        self.basis = None;
        self.candidate = false;
        self.effects = None;
        self.layout = Some(Arc::new(layout));
        self.milestones
            .scene
            .get_or_insert_with(|| timing::elapsed(self.clock));
        self.dirty = false;
        self.rounds = self.rounds.saturating_add(1);
        Ok(true)
    }

    pub(crate) fn finalize(&mut self) -> Result<bool, Fault> {
        if !self.candidate {
            return Ok(false);
        }
        let layout = self
            .layout
            .as_ref()
            .ok_or_else(|| Fault("layout candidate is absent".into()))?
            .clone();
        let routing = self
            .config
            .get("scene.routing")
            .map(bool::deserialize)
            .transpose()
            .map_err(error)?
            .unwrap_or(true);
        let checks = self
            .config
            .get("scene.checks")
            .map(usize::deserialize)
            .transpose()
            .map_err(error)?
            .unwrap_or(graphics::shared::CHECKS);
        let items = self.painter.build(&layout).map_err(error)?;
        self.computation.paints = self.computation.paints.saturating_add(1);
        self.computation.paint = self
            .computation
            .paint
            .saturating_add(self.painter.inspect().elapsed);
        let start = std::time::Instant::now();
        let source = graphics::shared::operation::Source {
            extent: layout.extent(),
            revision: layout.revision(),
            items,
        };
        let scene = if let Some(previous) = self.basis.as_ref().filter(|_| routing) {
            pipeline::step(graphics::shared::operation::Revision {
                previous,
                limit: checks,
            })
            .run(source)
        } else {
            pipeline::step(graphics::shared::operation::Publication).run(source)
        }
        .map_err(error)?;
        let elapsed = timing::elapsed(start);
        self.computation.scene = self.computation.scene.saturating_add(elapsed);
        self.timing.scene = self.timing.scene.saturating_add(elapsed);
        self.computation.finalizations = self.computation.finalizations.saturating_add(1);
        let tracks = layout.tracks(&self.capture.plans());
        if let Some(animation) = &self.animation {
            animation.replace(scene.generation(), tracks)?;
        } else {
            self.animator.replace(scene.generation(), tracks);
        }
        self.scene = Some(scene);
        self.basis = None;
        self.candidate = false;
        self.milestones
            .scene
            .get_or_insert_with(|| timing::elapsed(self.clock));
        Ok(true)
    }
}
