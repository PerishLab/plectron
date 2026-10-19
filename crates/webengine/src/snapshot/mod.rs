pub mod wire;
use crate::loading::Resource;
use crate::{Display, State};
use domengine::{Fragment, Item};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot<F = Fragment, I = Item> {
    #[serde(default)]
    pub computation: crate::Computation,
    #[serde(default)]
    pub relevance: crate::Relevance,
    #[serde(default)]
    pub compositing: crate::Compositing,
    #[serde(default)]
    pub mapping: crate::mapping::Work,
    #[serde(default)]
    pub milestones: crate::Milestones,
    pub execution: Option<crate::Execution>,
    pub url: String,
    pub state: State,
    pub fault: Option<String>,
    pub resources: Vec<Resource>,
    pub images: crate::resource::Images,
    pub revision: Option<u64>,
    pub painted: Option<u64>,
    pub presented: Option<u64>,
    pub frame: Option<u64>,
    pub scene: Option<u64>,
    pub displayed: Option<u64>,
    pub visual: graphics::shared::Visual,
    #[serde(default)]
    pub scrolling: Vec<crate::change::Scroll>,
    #[serde(default)]
    pub scrollwork: crate::change::Work,
    pub spaces: Vec<graphics::shared::Mapping>,
    pub shown: Option<graphics::shared::Visual>,
    pub issued: Option<u64>,
    pub pending: Option<u64>,
    pub output: Option<String>,
    pub visible: Vec<Region>,
    #[serde(default)]
    pub materialization: graphics::shared::Materialization,
    pub nodes: Arc<[Arc<Node>]>,
    pub focused: Option<usize>,
    pub caches: Vec<shared::cache::Snapshot>,
    pub reuse: domengine::Reuse,
    pub update: domengine::Update,
    pub painting: domengine::Painting,
    #[serde(default)]
    pub routing: graphics::shared::Routing,
    pub jobs: jsengine::Jobs,
    pub requests: crate::addons::Requests,
    #[serde(default)]
    pub bindings: crate::Bindings,
    #[serde(default)]
    pub activity: jsengine::Activity,
    pub performance: jsengine::jit::Report,
    pub events: Vec<crate::Dispatch>,
    pub fragments: Arc<[F]>,
    pub items: Vec<I>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub fragment: usize,
    pub space: u64,
    pub node: Option<usize>,
    pub rect: graphics::shared::Rect,
    pub clip: graphics::shared::Rect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: usize,
    pub parent: Option<usize>,
    pub kind: String,
    pub content: String,
    pub attributes: BTreeMap<String, String>,
    pub editor: Option<Editor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Editor {
    pub text: String,
    pub selection: domengine::Selection,
    pub composition: Option<domengine::ime::Composition>,
    pub caret: Option<usize>,
}

impl From<&Display> for Snapshot {
    fn from(page: &Display) -> Self {
        Self {
            execution: None,
            compositing: page.compositing,
            mapping: page.mapping,
            milestones: page.publication.milestones,
            url: page.publication.url.clone(),
            state: page.state(),
            fault: page.publication.fault.clone(),
            resources: page.publication.resources.clone(),
            images: page.publication.images.clone(),
            revision: page.publication.revision,
            presented: page.presented(),
            frame: page
                .presentation
                .visible
                .as_ref()
                .map(|view| view.frame.id()),
            issued: page
                .presentation
                .issued
                .as_ref()
                .map(|view| view.frame.id()),
            pending: page
                .presentation
                .pending
                .as_ref()
                .map(graphics::shared::Frame::id),
            scene: page.scene.as_ref().map(graphics::shared::Scene::id),
            displayed: page
                .presentation
                .visible
                .as_ref()
                .map(|view| view.frame.scene().id()),
            visual: page.visual.clone(),
            scrolling: page.scrolling(),
            scrollwork: page.scrollwork(),
            spaces: page
                .scene
                .as_ref()
                .map(|scene| {
                    scene
                        .mappings(&page.visual)
                        .expect("desired visual state is reconciled")
                })
                .unwrap_or_default(),
            shown: page
                .presentation
                .visible
                .as_ref()
                .map(|view| view.frame.visual().clone()),
            output: page.presentation.fault.clone(),
            visible: page
                .presentation
                .visible
                .as_ref()
                .map(|view| {
                    let mut regions = Vec::with_capacity(view.layout.root().count());
                    let mut index = 0;
                    view.layout.root().visit(&mut |local| {
                        let mapping = view
                            .frame
                            .mapping(local.space)
                            .expect("layout space is recorded");
                        regions.push(Region {
                            fragment: index,
                            space: local.space,
                            node: local.node,
                            rect: mapping.map(local.rect),
                            clip: mapping.clip,
                        });
                        index += 1;
                    });
                    regions
                })
                .unwrap_or_default(),
            materialization: page.materialization(),
            painted: page.layout.as_ref().map(|layout| layout.revision()),
            nodes: page.publication.nodes.clone(),
            focused: page.publication.focused,
            events: page.publication.events.clone(),
            performance: page.publication.performance.clone(),
            computation: page.publication.computation,
            relevance: page.publication.relevance,
            reuse: page.publication.reuse.clone(),
            update: page.publication.update.clone(),
            painting: page.publication.painting.clone(),
            routing: page
                .scene
                .as_ref()
                .map_or_else(Default::default, graphics::shared::Scene::routing),
            jobs: page.publication.jobs.clone(),
            requests: page.publication.requests.clone(),
            bindings: page.publication.bindings,
            activity: page.publication.activity,
            caches: page.publication.caches.clone(),
            fragments: page
                .layout
                .as_ref()
                .map(|layout| layout.records().clone())
                .unwrap_or_default(),
            items: page
                .scene
                .as_ref()
                .map(|scene| scene.items().to_vec())
                .unwrap_or_default(),
        }
    }
}

impl<F, I> Snapshot<F, I> {
    pub fn map<G, J>(self, fragments: Vec<G>, items: Vec<J>) -> Snapshot<G, J> {
        let Self {
            computation,
            relevance,
            compositing,
            mapping,
            milestones,
            execution,
            url,
            state,
            fault,
            resources,
            images,
            revision,
            painted,
            presented,
            frame,
            scene,
            displayed,
            visual,
            scrolling,
            scrollwork,
            spaces,
            shown,
            issued,
            pending,
            output,
            visible,
            materialization,
            nodes,
            focused,
            caches,
            reuse,
            update,
            painting,
            routing,
            jobs,
            requests,
            bindings,
            activity,
            performance,
            events,
            fragments: _,
            items: _,
        } = self;
        Snapshot {
            computation,
            relevance,
            compositing,
            mapping,
            milestones,
            execution,
            url,
            state,
            fault,
            resources,
            images,
            revision,
            painted,
            presented,
            frame,
            scene,
            displayed,
            visual,
            scrolling,
            scrollwork,
            spaces,
            shown,
            issued,
            pending,
            output,
            visible,
            materialization,
            nodes,
            focused,
            caches,
            reuse,
            update,
            painting,
            routing,
            jobs,
            requests,
            bindings,
            activity,
            performance,
            events,
            fragments: fragments.into(),
            items,
        }
    }
}
