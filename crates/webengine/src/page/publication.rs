use super::Content;
use crate::loading::Resource;
use crate::snapshot::Node;
use crate::{Fault, State};
use domengine::Layout;
use graphics::shared::Scene;
use std::sync::Arc;
const OBJECTS: u64 = 256;
const NODES: usize = 64;
enum Cause {
    Manual,
    Objects,
    Nodes,
    Both,
}
impl Cause {
    fn name(&self) -> &'static str {
        match self {
            Self::Manual => "web.collect.manual",
            Self::Objects => "web.collect.objects",
            Self::Nodes => "web.collect.nodes",
            Self::Both => "web.collect.both",
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct Publication {
    pub extent: domengine::Extent,
    pub computation: super::Computation,
    pub relevance: super::Relevance,
    pub milestones: super::Milestones,
    pub clock: std::time::Instant,
    pub url: String,
    pub state: State,
    pub fault: Option<String>,
    pub resources: Vec<Resource>,
    pub images: crate::resource::Images,
    pub revision: Option<u64>,
    pub nodes: Arc<[Arc<Node>]>,
    pub focused: Option<usize>,
    pub caches: Vec<shared::cache::Snapshot>,
    pub reuse: domengine::Reuse,
    pub update: domengine::Update,
    pub painting: domengine::Painting,
    pub jobs: jsengine::Jobs,
    pub requests: crate::addons::Requests,
    pub bindings: crate::Bindings,
    pub activity: jsengine::Activity,
    pub performance: jsengine::jit::Report,
    pub events: Vec<crate::Dispatch>,
    pub layout: Option<Arc<Layout>>,
    pub scene: Option<Scene>,
    pub effects: Option<graphics::shared::Effects>,
    pub scrolls: Vec<crate::display::Policy>,
    pub watching: Vec<domengine::Node>,
    pub replay: Vec<domengine::Node>,
}
impl Publication {
    pub fn empty(url: &str, extent: domengine::Extent) -> Result<Self, Fault> {
        Ok(Self {
            extent,
            computation: Default::default(),
            relevance: Default::default(),
            milestones: Default::default(),
            clock: std::time::Instant::now(),
            url: crate::resource::address(url)?.to_string(),
            state: State::Loading,
            fault: None,
            resources: Vec::new(),
            images: Default::default(),
            revision: None,
            nodes: Arc::default(),
            focused: None,
            events: Vec::new(),
            performance: Default::default(),
            reuse: Default::default(),
            update: Default::default(),
            painting: Default::default(),
            jobs: Default::default(),
            requests: Default::default(),
            bindings: Default::default(),
            activity: Default::default(),
            caches: Vec::new(),
            layout: None,
            scene: None,
            effects: None,
            scrolls: Vec::new(),
            watching: Vec::new(),
            replay: Vec::new(),
        })
    }
}
impl Content {
    pub(crate) fn collect(
        &mut self,
        roots: impl IntoIterator<Item = domengine::Node>,
    ) -> Result<super::Collection, Fault> {
        self.reclaim(roots, Cause::Manual)
    }
    fn reclaim(
        &mut self,
        roots: impl IntoIterator<Item = domengine::Node>,
        cause: Cause,
    ) -> Result<super::Collection, Fault> {
        observe::call(cause.name(), || self.sweep(roots))
    }
    fn sweep(
        &mut self,
        roots: impl IntoIterator<Item = domengine::Node>,
    ) -> Result<super::Collection, Fault> {
        self.check()?;
        let mut roots: Vec<_> = roots.into_iter().collect();
        if let Some(layout) = &self.layout {
            layout.roots(&mut |node| roots.push(node));
        }
        self.leases.retain(|lease| {
            let Some(layout) = lease.upgrade() else {
                return false;
            };
            layout.roots(&mut |node| roots.push(node));
            true
        });
        let sweep = self.gather(&mut roots)?;
        self.collected = sweep.allocation;
        if let Some(pictures) = &mut self.pictures {
            pictures.prune(&sweep.dead);
        }
        if !sweep.dead.is_empty() {
            let before = self.active.len();
            self.active.retain(|node| !sweep.dead.contains(node));
            if self.active.len() != before {
                self.pipeline
                    .demand(self.deferred.then(|| self.active.clone()));
            }
            self.dirty = true;
        }
        Ok(super::Collection {
            objects: sweep.objects,
            nodes: sweep.dead.len(),
            javascript: sweep.javascript,
            document: sweep.document,
        })
    }
    pub(crate) fn maintain(&mut self) -> Result<bool, Fault> {
        if self.state != State::Ready {
            return Ok(false);
        }
        let allocation = self.allocation()?;
        let objects = allocation.objects.saturating_sub(self.collected.objects) >= OBJECTS;
        let nodes = allocation.nodes.saturating_sub(self.collected.nodes) >= NODES;
        let cause = match (objects, nodes) {
            (true, true) => Some(Cause::Both),
            (true, false) => Some(Cause::Objects),
            (false, true) => Some(Cause::Nodes),
            (false, false) => None,
        };
        if let Some(cause) = cause {
            self.reclaim([], cause)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn publish(&mut self) -> Publication {
        let mut observation = observe::scope("web.snapshot");
        let relevance = self.relevance();
        if let Some(layout) = &self.layout {
            let lease = Arc::downgrade(layout);
            if self.leases.last().is_none_or(|last| !last.ptr_eq(&lease)) {
                self.leases.push(lease);
            }
        }
        let mut nodes = Arc::default();
        let mut revision = None;
        let mut focused = None;
        if let Some(runtime) = &mut self.runtime {
            let document = &runtime.addons().document;
            revision = Some(document.revision());
            focused = document.focused().map(|node| node.id());
            nodes = self.nodes.publish(document);
        }
        let mut publication = Publication {
            extent: self.extent,
            computation: self.computation,
            relevance,
            milestones: self.milestones,
            clock: self.clock,
            reuse: self.pipeline.inspect(),
            update: self.capture.inspect(),
            painting: self.painter.inspect(),
            jobs: self
                .runtime
                .as_ref()
                .map(|runtime| runtime.jobs())
                .unwrap_or_default(),
            requests: self
                .runtime
                .as_mut()
                .map(|runtime| runtime.addons().requests())
                .unwrap_or_default(),
            bindings: self
                .runtime
                .as_mut()
                .map(|runtime| runtime.addons().bindings())
                .unwrap_or_default(),
            caches: self.caches.inspect(),
            activity: self
                .runtime
                .as_ref()
                .map(|runtime| runtime.activity())
                .unwrap_or_default(),
            performance: self
                .runtime
                .as_ref()
                .map(|runtime| runtime.performance())
                .unwrap_or_default(),
            events: self
                .runtime
                .as_mut()
                .map(|runtime| runtime.addons().events.snapshot())
                .unwrap_or_default(),
            url: self.url.to_string(),
            state: self.state,
            fault: self.fault.clone(),
            resources: self.resources.clone(),
            images: self
                .pictures
                .as_ref()
                .map(|pictures| pictures.report.clone())
                .unwrap_or_default(),
            revision,
            nodes,
            focused,
            layout: self.layout.clone(),
            scene: self.scene.clone(),
            effects: self.effects.clone(),
            scrolls: self
                .runtime
                .as_mut()
                .map(|runtime| runtime.addons().scrolls.policies())
                .unwrap_or_default(),
            watching: self
                .runtime
                .as_mut()
                .map(|runtime| runtime.addons().watching())
                .unwrap_or_default(),
            replay: self
                .runtime
                .as_mut()
                .map(|runtime| runtime.addons().scrolls.replay())
                .unwrap_or_default(),
        };
        if self.scene.is_some() && !self.candidate {
            self.milestones
                .publication
                .get_or_insert_with(|| super::timing::elapsed(self.clock));
            publication.milestones = self.milestones;
        }
        observation.finish(true);
        publication
    }
    pub fn target(&mut self, node: Option<domengine::Node>) -> Result<(), Fault> {
        self.check()?;
        let Some(node) = node else {
            return crate::addons::change(self.runtime()?, None);
        };
        let runtime = self.runtime()?;
        if !runtime
            .addons()
            .document
            .connected(node)
            .map_err(crate::error)?
        {
            return Ok(());
        }
        crate::addons::click(runtime, node)
    }
    pub fn transition(
        &mut self,
        from: Option<domengine::Node>,
        to: Option<domengine::Node>,
    ) -> Result<(), Fault> {
        self.check()?;
        crate::addons::transition(self.runtime()?, from, to)
    }
}
