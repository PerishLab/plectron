use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loading {
    pub requests: u64,
    pub parsing: u64,
    pub registration: u64,
    pub fonts: usize,
    pub bytes: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Computation {
    #[serde(default)]
    pub animations: u64,
    #[serde(default)]
    pub effects: u64,
    #[serde(default)]
    pub coalesced: u64,
    #[serde(default)]
    pub discarded: u64,
    #[serde(default)]
    pub submitted: u64,
    #[serde(default)]
    pub mailbox: u64,
    #[serde(default)]
    pub sampling: u64,
    #[serde(default)]
    pub pulses: u64,
    #[serde(default)]
    pub superseded: u64,
    pub layouts: u64,
    #[serde(default)]
    pub projections: u64,
    #[serde(default)]
    pub finalizations: u64,
    pub paints: u64,
    pub checks: u64,
    pub changes: u64,
    pub queries: u64,
    pub capture: u64,
    pub measurement: u64,
    pub assembly: u64,
    #[serde(default)]
    pub projection: u64,
    pub paint: u64,
    pub scene: u64,
}
impl Computation {
    pub fn since(self, previous: Self) -> Self {
        Self {
            animations: self.animations - previous.animations,
            effects: self.effects - previous.effects,
            coalesced: self.coalesced - previous.coalesced,
            discarded: self.discarded - previous.discarded,
            submitted: self.submitted - previous.submitted,
            mailbox: self.mailbox,
            sampling: self.sampling - previous.sampling,
            pulses: self.pulses - previous.pulses,
            superseded: self.superseded - previous.superseded,
            layouts: self.layouts - previous.layouts,
            projections: self.projections - previous.projections,
            finalizations: self.finalizations - previous.finalizations,
            paints: self.paints - previous.paints,
            checks: self.checks - previous.checks,
            changes: self.changes - previous.changes,
            queries: self.queries - previous.queries,
            capture: self.capture - previous.capture,
            measurement: self.measurement - previous.measurement,
            assembly: self.assembly - previous.assembly,
            projection: self.projection - previous.projection,
            paint: self.paint - previous.paint,
            scene: self.scene - previous.scene,
        }
    }
    pub(super) fn measure(&mut self, capture: &domengine::Update, layout: &domengine::Reuse) {
        self.layouts += 1;
        self.capture += capture.elapsed;
        self.measurement += layout.measurement;
        self.assembly += layout.assembly;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timing {
    pub content: u64,
    #[serde(default)]
    pub checkpoint: u64,
    #[serde(default)]
    pub collections: u64,
    #[serde(default)]
    pub collection: u64,
    pub scene: u64,
    pub snapshot: u64,
    pub install: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Milestones {
    #[serde(default)]
    pub resources: Option<u64>,
    #[serde(default)]
    pub fonts: Option<u64>,
    #[serde(default)]
    pub pictures: Option<u64>,
    pub loaded: Option<u64>,
    pub runtime: Option<u64>,
    pub activation: Option<u64>,
    pub scene: Option<u64>,
    pub publication: Option<u64>,
    pub frame: Option<u64>,
    pub presentation: Option<u64>,
}
pub(crate) fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

pub(crate) fn mark(slot: &mut Option<u64>, start: Instant) {
    slot.get_or_insert(elapsed(start));
}
