pub mod summary;

use super::source::Asset;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct Report {
    pub repetitions: usize,
    pub case: super::Case,
    pub size: String,
    pub count: usize,
    pub edits: usize,
    pub backend: String,
    pub mode: String,
    pub architecture: &'static str,
    pub system: &'static str,
    pub debug: bool,
    pub unit: &'static str,
    pub rasterization: &'static str,
    pub endpoint: &'static str,
    pub accounting: &'static str,
    pub capture: &'static str,
    pub summary: BTreeMap<String, summary::Summary>,
    pub runs: Vec<Run>,
}
#[derive(Serialize)]
pub struct Run {
    pub config: config::Entries,
    pub load: u64,
    pub startup: webengine::Milestones,
    pub loading: webengine::Loading,
    pub assets: BTreeMap<String, Asset>,
    pub stages: Vec<Stage>,
    pub closed: bool,
    pub failure: Option<Failure>,
}
#[derive(Serialize)]
pub struct Stage {
    pub activity: jsengine::Activity,
    pub bindings: webengine::Bindings,
    pub performance: jsengine::jit::Report,
    pub computation: webengine::Computation,
    pub relevance: webengine::Relevance,
    pub action: String,
    pub trigger: &'static str,
    pub evaluation: u64,
    pub publication: u64,
    pub inspection: u64,
    pub publishing: webengine::Timing,
    pub total: u64,
    pub capture: Option<super::rendering::Capture>,
    pub geometry: super::check::Geometry,
    pub rendering: Option<Rendering>,
    pub nodes: usize,
    pub records: usize,
    pub depth: usize,
    pub characters: usize,
    pub visible: usize,
    pub update: Option<domengine::Update>,
    pub reuse: Option<domengine::Reuse>,
    pub painting: Option<domengine::Painting>,
    pub routing: Option<graphics::shared::Routing>,
    pub caches: Vec<shared::cache::Snapshot>,
    pub images: webengine::resource::Images,
}
#[derive(Default, Serialize)]
pub struct Rendering {
    pub bounds: Option<shared::cache::Snapshot>,
    pub pruning: Option<shared::cache::Snapshot>,
    pub work: graphics::shared::preparation::Work,
    pub preparation: u64,
    pub submission: u64,
    pub completion: u64,
    pub readback: u64,
    pub gpu: Option<u64>,
    pub builds: u64,
    pub uploads: Option<u64>,
    pub textures: Option<u64>,
    pub groups: usize,
    pub maximum: u32,
    pub checksum: u64,
}

#[derive(Serialize)]
pub struct Failure {
    pub action: String,
    pub reason: String,
    pub nodes: usize,
    pub records: usize,
}
