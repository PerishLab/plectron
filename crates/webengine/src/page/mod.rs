mod animation;
mod content;
mod demand;
mod interaction;
mod nodes;
pub mod operation;
mod publication;
mod timing;
use crate::resource::Provider;
use crate::{Display, Fault, Hit, Snapshot};
pub(crate) use animation::spawn as spawn_animation;
pub(crate) use animation::{Controller as AnimationController, Reader as AnimationReader};
pub(crate) use content::{Candidate, Content};
pub use demand::Relevance;
use domengine::Extent;
pub(crate) use publication::Publication;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
pub(crate) use timing::elapsed;
pub use timing::{Computation, Loading, Milestones, Timing};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Loading,
    Ready,
    Failed,
    Closed,
}

pub struct Page {
    content: Content,
    display: Display,
    timing: Timing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Collection {
    pub objects: usize,
    pub nodes: usize,
    pub javascript: jsengine::Collection,
    pub document: domengine::Collection,
}
impl std::fmt::Display for Collection {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            output,
            "objects={} nodes={} js_live={} js_capacity={} js_vacant={} js_weak={} js_edges={} js_activations={} js_rounds={} dom_live={} dom_capacity={} dom_vacant={} dom_allocations={}",
            self.objects,
            self.nodes,
            self.javascript.live,
            self.javascript.capacity,
            self.javascript.vacant,
            self.javascript.weak,
            self.javascript.edges,
            self.javascript.activations,
            self.javascript.rounds,
            self.document.live,
            self.document.capacity,
            self.document.vacant,
            self.document.allocations,
        )
    }
}
impl Page {
    pub fn new(url: &str, extent: Extent) -> Result<Self, Fault> {
        Self::configured(url, extent, config::Store::default())
    }
    pub fn configured(url: &str, extent: Extent, config: config::Store) -> Result<Self, Fault> {
        Ok(Self {
            content: Content::new(url, extent, config)?,
            display: Display::new(url, extent)?,
            timing: Timing::default(),
        })
    }
    pub fn timing(&self) -> Timing {
        self.timing
    }
    pub fn loading(&self) -> Loading {
        self.content.loading
    }
    pub fn fonts(&self) -> textengine::Catalog {
        self.content.fonts.clone()
    }
    pub fn load(&mut self, provider: &mut impl Provider) -> Result<(), Fault> {
        let result = self.content.load(provider);
        self.publish()?;
        result
    }
    pub fn poll(&mut self) -> Result<bool, Fault> {
        let result = self.content.poll();
        self.publish()?;
        result
    }
    pub fn pulse(&mut self, pulse: graphics::shared::Pulse) -> Result<bool, Fault> {
        let changed = self.content.pulse(pulse)?;
        if changed {
            self.publish()?;
        }
        Ok(changed)
    }
    pub fn pacing(&self) -> graphics::shared::Pacing {
        match self
            .content
            .animator
            .schedule(timing::elapsed(self.content.clock))
        {
            Some(domengine::style::animation::Schedule::Continuous) => {
                graphics::shared::Pacing::Continuous
            }
            Some(domengine::style::animation::Schedule::At(at)) => self
                .content
                .clock
                .checked_add(std::time::Duration::from_micros(at))
                .map_or(graphics::shared::Pacing::Idle, graphics::shared::Pacing::At),
            None => graphics::shared::Pacing::Idle,
        }
    }
    pub fn render(&mut self) -> Result<(), Fault> {
        observe::call("web.render", || self.compose())
    }
    fn compose(&mut self) -> Result<(), Fault> {
        self.timing = Timing::default();
        self.notify()?;
        let start = std::time::Instant::now();
        let result = self.content.render();
        self.timing.content = timing::elapsed(start);
        self.timing.checkpoint = self.content.timing.checkpoint;
        self.timing.collections = self.content.timing.collections;
        self.timing.collection = self.content.timing.collection;
        result?;
        let start = std::time::Instant::now();
        let candidate = self.content.candidate();
        let publication = candidate.is_none().then(|| self.content.publish());
        self.timing.snapshot = timing::elapsed(start);
        let start = std::time::Instant::now();
        let result = self
            .sync(candidate, publication)
            .and_then(|()| self.settle());
        let result = result.and_then(|()| self.finalize());
        self.timing.install = timing::elapsed(start);
        self.timing.scene = self.content.timing.scene;
        result
    }
    pub fn cache(&mut self, id: u64, command: shared::cache::Command) -> Result<String, Fault> {
        let result = self.content.cache(id, command);
        self.publish()?;
        result
    }
    pub fn collect(&mut self) -> Result<Collection, Fault> {
        let mut roots = Vec::new();
        self.display.roots(&mut |node| roots.push(node));
        self.content.collect(roots)
    }
    pub fn tier(&mut self, mode: jsengine::jit::Mode) -> Result<(), Fault> {
        self.content.check()?;
        self.content.runtime()?.tier(mode);
        Ok(())
    }
    pub fn evaluate(&mut self, source: &str) -> Result<String, Fault> {
        observe::call("web.evaluate", || {
            self.notify()?;
            self.content.evaluate(source)
        })
    }
    pub fn dispatch(&mut self, hit: &Hit) -> Result<(), Fault> {
        self.notify()?;
        if let Some(node) = hit.node() {
            let document = &self.content.runtime()?.addons().document;
            if !document.owns(node) {
                return Err(Fault("node belongs to another document".into()));
            }
            if document.entry(node).is_err() {
                return Ok(());
            }
        }
        self.content.target(hit.node())
    }
    pub fn transition(&mut self, from: Option<&Hit>, to: Option<&Hit>) -> Result<(), Fault> {
        self.notify()?;
        self.content
            .transition(from.and_then(Hit::node), to.and_then(Hit::node))?;
        self.render()
    }
    pub fn click(&mut self, x: usize, y: usize) -> Result<(), Fault> {
        self.notify()?;
        self.content.target(self.display.pick(x, y))
    }
    pub fn input(
        &mut self,
        input: &crate::keyboard::Input,
    ) -> Result<crate::keyboard::Outcome, Fault> {
        self.notify()?;
        self.content.input(input)
    }
    pub fn edit(&mut self, edit: &crate::Edit) -> Result<(), Fault> {
        self.notify()?;
        self.content.edit(edit)
    }
    pub fn composition(&mut self, input: &crate::ime::Input) -> Result<bool, Fault> {
        self.notify()?;
        self.content.compose(input)
    }
    pub fn receive(&mut self, message: String) -> Result<(), Fault> {
        self.notify()?;
        self.content.receive(message)
    }
    pub fn messages(&mut self) -> Vec<String> {
        self.content.messages()
    }
    pub fn inspect(&mut self) -> Snapshot {
        if self.display.state() != State::Closed {
            self.publish().expect("local publication is valid");
        }
        self.display.inspect()
    }
    pub fn close(&mut self) {
        self.content.close();
        self.display.close();
    }
    fn publish(&mut self) -> Result<(), Fault> {
        self.sync(None, None)?;
        self.settle()?;
        self.finalize()
    }
    fn notify(&mut self) -> Result<(), Fault> {
        let changes = self.display.take();
        if changes.is_empty() {
            return Ok(());
        }
        self.content.scrolling(changes)
    }
    fn sync(
        &mut self,
        candidate: Option<Candidate>,
        publication: Option<Publication>,
    ) -> Result<(), Fault> {
        if let Some(candidate) = candidate.or_else(|| self.content.candidate()) {
            self.display.project(candidate)
        } else {
            self.display
                .install(publication.unwrap_or_else(|| self.content.publish()))
        }
    }
    fn finalize(&mut self) -> Result<(), Fault> {
        for _ in 0..=32 {
            if !self.content.finalize()? {
                return Ok(());
            }
            self.sync(None, None)?;
            self.settle()?;
        }
        Err(Fault("final scene did not converge".into()))
    }
    fn settle(&mut self) -> Result<(), Fault> {
        for _ in 0..=32 {
            if self.content.visual == self.display.visual {
                return Ok(());
            }
            if self.content.viewport(self.display.visual.clone())? {
                self.sync(None, None)?;
            }
        }
        Err(Fault("visual demand did not converge".into()))
    }
}
impl Deref for Page {
    type Target = Display;
    fn deref(&self) -> &Display {
        &self.display
    }
}

impl Page {
    pub fn frame(&mut self) -> Option<graphics::shared::Frame> {
        self.display.frame()
    }
    pub fn acknowledge(&mut self, frame: u64) -> Result<bool, Fault> {
        self.display.acknowledge(frame)
    }
    pub fn complete(&mut self, frame: u64, result: Result<(), Fault>) -> Result<bool, Fault> {
        self.display.complete(frame, result)
    }
    pub fn scroll(&mut self, delta: i64) -> Result<(), Fault> {
        self.display.scroll(delta)?;
        self.viewport()
    }
    pub fn pan(&mut self, scene: u64, space: u64, delta: i64) -> Result<(), Fault> {
        self.display.pan(scene, space, delta)?;
        self.viewport()
    }
    pub fn wheel(&mut self, x: usize, y: usize, delta: i64) -> Result<crate::Disposition, Fault> {
        observe::call("web.wheel", || {
            let result = self.display.wheel(x, y, delta)?;
            if result.applied() {
                observe::call("web.viewport", || self.viewport())?;
            }
            Ok(result)
        })
    }
}
