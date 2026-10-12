mod change;
mod control;
mod mapping;
mod presentation;
mod visual;
use crate::page::Publication;
use crate::{Fault, Snapshot, State};
pub(crate) use change::Change;
pub use change::{
    Boundary as ScrollEdge, Cause as ScrollCause, Position as ScrollPosition,
    Scroll as ScrollChange, Work as ScrollWork,
};
pub(crate) use control::{Action, Bookmark, Edge, Policy, Position as BookmarkPosition};
use domengine::{Extent, Layout, Node};
use graphics::shared::{Frame, Scene, Visual};
pub use mapping::Work as MappingWork;
pub use presentation::Hit;
use presentation::Presentation;
use std::collections::BTreeMap;
use std::sync::Arc;
pub use visual::Disposition;

pub struct Display {
    pub(crate) publication: Publication,
    pub(crate) layout: Option<Arc<Layout>>,
    pub(crate) scene: Option<Scene>,
    pub(crate) visual: Visual,
    pub(crate) presentation: Presentation,
    pub(crate) compositing: Compositing,
    pub(crate) mapping: mapping::Work,
    scrolls: BTreeMap<u64, Control>,
    watching: std::collections::BTreeSet<Node>,
    journal: change::Journal,
    effect: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Compositing {
    pub updates: u64,
    pub effects: u64,
    pub stale: u64,
    pub elapsed: u64,
}

#[derive(Debug, Clone)]
pub struct Focus {
    pub target: usize,
    pub text: String,
    pub selection: domengine::Selection,
    pub caret: graphics::shared::Rect,
}

#[derive(Clone, Copy)]
struct Control {
    node: Node,
    version: u64,
    space: Option<u64>,
    action: Action,
    active: bool,
}

impl Display {
    pub(crate) fn roots(&self, visit: &mut dyn FnMut(Node)) {
        if let Some(layout) = &self.layout {
            layout.roots(visit);
        }
        for view in [&self.presentation.visible, &self.presentation.issued]
            .into_iter()
            .flatten()
        {
            view.layout.roots(visit);
        }
    }

    pub fn new(url: &str, extent: Extent) -> Result<Self, Fault> {
        extent.area().map_err(crate::error)?;
        Ok(Self {
            publication: Publication::empty(url, extent)?,
            layout: None,
            scene: None,
            visual: Visual::new(extent),
            presentation: Presentation::default(),
            compositing: Compositing::default(),
            mapping: mapping::Work::default(),
            scrolls: BTreeMap::new(),
            watching: Default::default(),
            journal: Default::default(),
            effect: None,
        })
    }
    pub(crate) fn install(&mut self, publication: Publication) -> Result<(), Fault> {
        if self.state() == State::Closed {
            return Err(Fault("display closed".into()));
        }
        self.visual.extent = publication.extent;
        let replaced = self.publication.scene.as_ref().map(Scene::id)
            != publication.scene.as_ref().map(Scene::id);
        let stable = control::unchanged(&self.publication, &publication);
        let frame = self.publication.milestones.frame;
        let presentation = self.publication.milestones.presentation;
        self.watching = publication.watching.iter().copied().collect();
        let controlled = if stable {
            false
        } else {
            self.policies(
                &publication.scrolls,
                publication.layout.as_deref(),
                publication.scene.as_ref(),
                &publication.replay,
            )?
        };
        if replaced {
            self.replace(&publication);
        }
        self.layout = publication.layout.clone();
        self.publication = publication;
        self.publication.milestones.frame = frame.or(self.publication.milestones.frame);
        self.publication.milestones.presentation =
            presentation.or(self.publication.milestones.presentation);
        let effected = if let Some(update) = self.publication.effects.clone() {
            self.compose(update)?
        } else {
            false
        };
        if (replaced || effected || controlled) && self.scene.is_some() {
            self.refresh()?;
        }
        Ok(())
    }
    fn compose(&mut self, update: graphics::shared::Effects) -> Result<bool, Fault> {
        if self.effect == Some(update.id()) {
            return Ok(false);
        }
        self.effect = Some(update.id());
        self.compositing.updates = self.compositing.updates.saturating_add(1);
        self.compositing.effects = self
            .compositing
            .effects
            .saturating_add(update.values().len() as u64);
        let Some(scene) = &self.scene else {
            self.compositing.stale = self.compositing.stale.saturating_add(1);
            return Ok(false);
        };
        if scene.generation() != update.generation() {
            self.compositing.stale = self.compositing.stale.saturating_add(1);
            return Ok(false);
        }
        let start = std::time::Instant::now();
        let changed = scene.animate(&update).map_err(crate::error)?;
        self.compositing.elapsed = self
            .compositing
            .elapsed
            .saturating_add(crate::page::elapsed(start));
        if let Some(scene) = changed {
            self.scene = Some(scene);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub(crate) fn effect(&mut self, update: graphics::shared::Effects) -> Result<(), Fault> {
        if self.compose(update)? && self.scene.is_some() {
            self.refresh()?;
        }
        Ok(())
    }
    pub(crate) fn reset(&mut self) -> Result<(), Fault> {
        if self.effect.take().is_none() {
            return Ok(());
        }
        self.scene = self.publication.scene.clone();
        if self.scene.is_none() {
            return Ok(());
        }
        self.refresh()
    }
    pub fn state(&self) -> State {
        self.publication.state
    }
    pub fn frame(&mut self) -> Option<Frame> {
        if let (Some(layout), Some(frame)) = (&self.layout, &self.presentation.pending)
            && !layout.ready(frame)
        {
            return None;
        }
        let frame = self.presentation.frame(&self.layout);
        if frame.is_some() {
            self.publication
                .milestones
                .frame
                .get_or_insert_with(|| crate::page::elapsed(self.publication.clock));
        }
        frame
    }
    pub fn acknowledge(&mut self, frame: u64) -> Result<bool, Fault> {
        self.complete(frame, Ok(()))
    }
    pub fn complete(&mut self, frame: u64, result: Result<(), Fault>) -> Result<bool, Fault> {
        self.check()?;
        let first = self.presentation.complete(frame, result)?;
        if first {
            self.publication
                .milestones
                .presentation
                .get_or_insert_with(|| crate::page::elapsed(self.publication.clock));
        }
        Ok(first)
    }
    pub fn presented(&self) -> Option<u64> {
        self.presentation
            .visible
            .as_ref()
            .map(|view| view.layout.revision())
    }
    pub fn inspect(&self) -> Snapshot {
        Snapshot::from(self)
    }

    pub fn materialization(&self) -> graphics::shared::Materialization {
        self.presentation
            .pending
            .as_ref()
            .or_else(|| self.presentation.issued.as_ref().map(|view| &view.frame))
            .or_else(|| self.presentation.visible.as_ref().map(|view| &view.frame))
            .map_or_else(Default::default, Frame::materialization)
    }

    pub fn focus(&self, frame: &Frame) -> Option<Focus> {
        let focused = self.publication.focused?;
        let editor = self
            .publication
            .nodes
            .iter()
            .find(|node| node.id == focused)?
            .editor
            .as_ref()?;
        let layout = self.layout.as_ref()?;
        let caret = editor.caret;
        let caret = layout.fragments().iter().find_map(|fragment| {
            if cursor(fragment, caret) {
                layout
                    .mapping(frame, fragment)
                    .map(|mapping| mapping.map(fragment.rect))
            } else {
                None
            }
        })?;
        Some(Focus {
            target: focused,
            text: editor.text.clone(),
            selection: editor.selection,
            caret,
        })
    }
    pub fn close(&mut self) {
        self.publication.state = State::Closed;
        self.publication.nodes = Arc::default();
        self.publication.focused = None;
        self.publication.revision = None;
        self.publication.layout = None;
        self.publication.scene = None;
        self.layout = None;
        self.scene = None;
        self.scrolls.clear();
        self.watching.clear();
        self.journal = Default::default();
        self.effect = None;
        self.mapping = Default::default();
        self.presentation = Presentation::default();
    }
    pub(crate) fn pick(&self, x: usize, y: usize) -> Option<Node> {
        if let Some(view) = &self.presentation.visible {
            return view.hit(x, y);
        }
        self.presentation
            .pending
            .as_ref()
            .or_else(|| self.presentation.issued.as_ref().map(|view| &view.frame))
            .and_then(|frame| {
                self.layout
                    .as_ref()
                    .and_then(|layout| layout.pick(frame, x, y))
            })
    }
    pub(crate) fn check(&self) -> Result<(), Fault> {
        if self.state() == State::Ready {
            Ok(())
        } else {
            Err(Fault("content is not ready".into()))
        }
    }
}

fn cursor(fragment: &domengine::Fragment, caret: Option<usize>) -> bool {
    match fragment.content {
        domengine::Content::Caret { .. } => true,
        domengine::Content::Box { .. } => caret.is_some_and(|node| fragment.node == Some(node)),
        _ => false,
    }
}
