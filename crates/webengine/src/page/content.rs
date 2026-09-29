use crate::addons::Web;
use crate::loading::Resource;
use crate::resource::{Provider, Url, address};
use crate::{Fault, State, error};
use domengine::{Capture, Extent, Layout, Painter, Pipeline, Sheet, style::Resolver};
use graphics::shared::Scene;
use jsruntime::Runtime;
use serde::Deserialize;
use std::sync::{Arc, Weak};
use textengine::Catalog;
pub(crate) mod convergence;
mod loading;
mod visual;
pub(crate) use convergence::Candidate;
use convergence::Convergence;
pub(crate) struct Content {
    pub(super) computation: super::Computation,
    pub(super) animator: super::animation::Animator,
    pub(super) animation: Option<super::animation::Controller>,
    pub(super) milestones: super::Milestones,
    pub(super) visual: graphics::shared::Visual,
    pub(super) active: std::collections::BTreeSet<domengine::Node>,
    pub(super) dirty: bool,
    pub(super) deferred: bool,
    pub(super) overscan: usize,
    config: config::Store,
    pub(crate) url: Url,
    pub(crate) state: State,
    pub(crate) fault: Option<String>,
    pub(crate) runtime: Option<Runtime<Web>>,
    pub(crate) layout: Option<Arc<Layout>>,
    pub(crate) leases: Vec<Weak<Layout>>,
    pub(crate) collected: (u64, usize),
    pub(crate) resources: Vec<Resource>,
    pub(super) nodes: super::nodes::Nodes,
    pub(super) extent: Extent,
    pub(crate) scene: Option<Scene>,
    basis: Option<Scene>,
    pub(super) candidate: bool,
    convergence: Convergence,
    rounds: usize,
    pub(crate) effects: Option<graphics::shared::Effects>,
    pub(crate) capture: Capture,
    pub(crate) caches: shared::cache::Manager,
    pub(crate) pipeline: Pipeline<Catalog>,
    pub(crate) painter: Painter,
    pub(crate) fonts: Catalog,
    epoch: u64,
    pub(super) version: u64,
    pub(super) clock: std::time::Instant,
    pub(super) timing: super::Timing,
    pub(super) loading: super::Loading,
    pub(super) pictures: Option<crate::resource::Pictures>,
}

impl Content {
    pub fn new(url: &str, extent: Extent, config: config::Store) -> Result<Self, Fault> {
        Self::configured(url, extent, config, (std::time::Instant::now(), None))
    }

    pub(crate) fn threaded(
        url: &str,
        extent: Extent,
        config: config::Store,
        threading: (std::time::Instant, super::animation::Controller),
    ) -> Result<Self, Fault> {
        Self::configured(url, extent, config, (threading.0, Some(threading.1)))
    }

    fn configured(
        url: &str,
        extent: Extent,
        config: config::Store,
        threading: (std::time::Instant, Option<super::animation::Controller>),
    ) -> Result<Self, Fault> {
        let (clock, animation) = threading;
        extent.area().map_err(error)?;
        let caches = shared::cache::Manager::default();
        let deferred = config
            .get("layout.demand")
            .map(bool::deserialize)
            .transpose()
            .map_err(error)?
            .unwrap_or(true);
        let overscan = config
            .get("layout.overscan")
            .map(usize::deserialize)
            .transpose()
            .map_err(error)?
            .unwrap_or(extent.height / 2);
        if overscan > 4096 {
            return Err(Fault("layout overscan exceeds viewport budget".into()));
        }
        let convergence = match config
            .get("layout.convergence")
            .map(String::deserialize)
            .transpose()
            .map_err(error)?
            .as_deref()
        {
            None | Some("adaptive") => Convergence::Adaptive,
            Some("eager") => Convergence::Eager,
            Some("deferred") => Convergence::Deferred,
            Some(_) => return Err(Fault("unknown layout convergence mode".into())),
        };
        let fonts = Catalog::configured(&caches, config.clone()).map_err(error)?;
        let mut pipeline =
            Pipeline::configured(fonts.clone(), &caches, config.clone()).map_err(error)?;
        pipeline.demand(deferred.then(Default::default));
        let painter = Painter::configured(&caches, config.clone()).map_err(error)?;
        let styles = Resolver::managed(Sheet::default(), &caches).map_err(error)?;
        Ok(Self {
            computation: Default::default(),
            animator: Default::default(),
            animation,
            milestones: Default::default(),
            visual: graphics::shared::Visual::new(extent),
            active: Default::default(),
            dirty: false,
            deferred,
            overscan,
            config,
            url: address(url)?,
            extent,
            scene: None,
            basis: None,
            candidate: false,
            convergence,
            rounds: 0,
            effects: None,
            state: State::Loading,
            fault: None,
            runtime: None,
            layout: None,
            leases: Vec::new(),
            collected: (0, 0),
            resources: Vec::new(),
            nodes: Default::default(),
            capture: Capture::new(styles),
            caches,
            pipeline,
            painter,
            fonts,
            epoch: 0,
            version: 0,
            clock,
            timing: Default::default(),
            loading: Default::default(),
            pictures: None,
        })
    }

    pub fn load(&mut self, provider: &mut impl Provider) -> Result<(), Fault> {
        if self.state != State::Loading {
            return Err(Fault("content can only be loaded once".into()));
        }
        if let Err(fault) = self.start(provider) {
            self.state = State::Failed;
            self.fault = Some(fault.to_string());
            return Err(fault);
        }
        self.state = State::Ready;
        Ok(())
    }

    pub fn cache(&mut self, id: u64, command: shared::cache::Command) -> Result<String, Fault> {
        self.check()?;
        let generation = self.caches.command(id, command).map_err(error)?;
        if let Some(runtime) = &mut self.runtime {
            runtime.addons().maintain();
        }
        self.fonts.maintain();
        self.pipeline.maintain();
        self.painter.maintain();
        self.capture.maintain();
        Ok(format!("cache={id} applied={generation}"))
    }

    pub fn close(&mut self) {
        let mut collection = observe::scope("web.collect.shutdown");
        self.state = State::Closed;
        self.runtime = None;
        self.nodes = Default::default();
        self.pictures = None;
        self.pipeline.close();
        self.fonts.close();
        self.painter.close();
        self.capture.close();
        self.animator = Default::default();
        self.animation = None;
        self.layout = None;
        self.leases.clear();
        self.scene = None;
        self.basis = None;
        self.candidate = false;
        self.rounds = 0;
        self.effects = None;
        collection.finish(true);
    }

    pub(crate) fn runtime(&mut self) -> Result<&mut Runtime<Web>, Fault> {
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| Fault("render runtime is absent".into()))?;
        runtime
            .addons()
            .document
            .configure(&self.config)
            .map_err(error)?;
        Ok(runtime)
    }

    pub(crate) fn check(&self) -> Result<(), Fault> {
        if self.state == State::Ready {
            Ok(())
        } else {
            Err(Fault("content is not ready".into()))
        }
    }
}
