use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Activity {
    Request,
    Retain,
    Hit,
    Window,
    Carry,
    Miss,
    Cold,
    Dependency,
    Budget,
    Disabled,
    Compute,
    Bypass,
    Screen,
    Store,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Category {
    Block,
    Flex,
    Text,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub identity: u64,
    pub parent: Option<u64>,
    pub fingerprint: u64,
    pub request: u64,
    pub activity: Activity,
    pub category: Category,
    pub work: usize,
    pub weight: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Causality {
    capacity: usize,
    dropped: usize,
    events: Vec<Event>,
}

impl Causality {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            dropped: 0,
            events: Vec::with_capacity(capacity),
        }
    }

    pub fn clear(&mut self) {
        self.dropped = 0;
        self.events.clear();
    }

    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub(in crate::layout) fn record(
        &mut self,
        element: crate::layout::input::Entry<'_>,
        constraints: crate::layout::Constraints,
        state: (Activity, Category),
        work: usize,
    ) {
        if self.events.len() == self.capacity {
            self.dropped += 1;
            return;
        }
        let mut digest = ahash::AHasher::default();
        (element.key.fingerprint(), constraints).hash(&mut digest);
        self.events.push(Event {
            identity: element.id().value(),
            parent: element.parent().map(geoengine::Id::value),
            fingerprint: element.key.fingerprint(),
            request: digest.finish(),
            activity: state.0,
            category: state.1,
            work,
            weight: element.key.weight,
        });
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reuse {
    #[serde(default)]
    pub dependencies: usize,
    #[serde(default)]
    pub isolated: usize,
    #[serde(default)]
    pub deferred: usize,
    #[serde(default)]
    pub estimation: Estimation,
    #[serde(default)]
    pub construction: Construction,
    #[serde(default)]
    pub intrinsic: crate::layout::Intrinsic,
    #[serde(default)]
    pub flex: Sizing,
    #[serde(default)]
    pub operations: Operations,
    #[serde(default)]
    pub retained: usize,
    #[serde(default)]
    pub planned: usize,
    pub bypassed: usize,
    #[serde(default)]
    pub screened: usize,
    #[serde(default)]
    pub screenwork: usize,
    #[serde(default)]
    pub screenmax: usize,
    #[serde(default)]
    pub screenbytes: usize,
    #[serde(default)]
    pub screenpeak: usize,
    #[serde(default)]
    pub windowed: usize,
    #[serde(default)]
    pub windowpeak: usize,
    #[serde(default)]
    pub windowbytes: usize,
    pub measurement: u64,
    pub assembly: u64,
    pub shaping: super::Shaping,
    pub enabled: bool,
    pub disabled: usize,
    pub revision: u64,
    pub fragments: usize,
    pub reused: usize,
    #[serde(default)]
    pub relocated: usize,
    pub requests: usize,
    pub hits: usize,
    pub computed: usize,
    pub cold: usize,
    pub budgets: usize,
    pub evicted: usize,
    pub omitted: usize,
    #[serde(default, alias = "work")]
    pub charged: usize,
    pub entries: usize,
    pub bytes: usize,
    pub reset: bool,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Estimation {
    pub resolved: usize,
    pub error: usize,
    pub maximum: usize,
    pub entries: usize,
    pub bytes: usize,
    pub evicted: usize,
    pub omitted: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Operations {
    pub nodes: usize,
    pub blocks: usize,
    pub flexes: usize,
    pub texts: usize,
    pub images: usize,
    pub minimums: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sizing {
    pub cross: Extremes,
    pub base: usize,
    pub minimum: usize,
    pub projected: usize,
    pub resolved: usize,
    pub carried: usize,
    pub stretch: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extremes {
    pub maximum: usize,
    pub minimum: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Construction {
    #[serde(default)]
    pub text: Materialization,
    #[serde(default)]
    pub analysis: Analysis,
    pub requests: usize,
    pub branches: usize,
    pub fragments: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Materialization {
    #[serde(default)]
    pub cached: usize,
    pub requests: usize,
    pub retained: usize,
    pub generated: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis {
    pub checked: usize,
    pub resized: usize,
    pub texts: usize,
    pub stable: usize,
    pub wrapped: usize,
}

impl Analysis {
    pub(super) fn compare(
        &mut self,
        before: &crate::layout::measure::Basis,
        after: &crate::layout::measure::View<'_>,
    ) {
        self.checked += 1;
        self.resized += usize::from(before.size() != after.size());
        for (a, b) in before.texts().iter().zip(after.children()) {
            if let (Some(a), Some(b)) = (a, b.text) {
                self.texts += 1;
                self.stable += usize::from(a.same(b));
                self.wrapped += usize::from(
                    !a.ranges
                        .iter()
                        .map(|line| &line.source)
                        .eq(b.ranges.iter().map(|line| &line.source)),
                );
            }
        }
    }
}
