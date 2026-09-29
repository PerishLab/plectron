use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STARTUP: &[(&str, Option<&str>, &str)] = &[
    ("resources", None, "resources"),
    ("fonts", Some("resources"), "fonts"),
    ("pictures", Some("fonts"), "pictures"),
    ("loading", Some("pictures"), "loaded"),
    ("runtime", Some("loaded"), "runtime"),
    ("activation", Some("runtime"), "activation"),
    ("scene", Some("activation"), "scene"),
    ("publication", Some("scene"), "publication"),
    ("load", None, "load"),
];
pub const LOADING: &[(&str, &str)] = &[
    ("fontrequest", "/loading/requests"),
    ("fontparse", "/loading/parsing"),
    ("fontregister", "/loading/registration"),
];

pub fn startup(run: &Value, start: Option<&str>, end: &str) -> Option<u64> {
    if end == "load" {
        return run.get("load")?.as_u64();
    }
    let end = run.pointer(&format!("/startup/{end}"))?.as_u64()?;
    let start = match start {
        Some(name) => run.pointer(&format!("/startup/{name}"))?.as_u64()?,
        None => 0,
    };
    end.checked_sub(start)
}

pub const TIMES: &[(&str, &str)] = &[
    ("captures", "/computation/capture"),
    ("measurements", "/computation/measurement"),
    ("assemblies", "/computation/assembly"),
    ("projections", "/computation/projection"),
    ("paints", "/computation/paint"),
    ("scenes", "/computation/scene"),
    ("total", "/total"),
    ("evaluation", "/evaluation"),
    ("publication", "/publication"),
    ("inspection", "/inspection"),
    ("content", "/publishing/content"),
    ("checkpoint", "/publishing/checkpoint"),
    ("collection", "/publishing/collection"),
    ("scene", "/publishing/scene"),
    ("snapshot", "/publishing/snapshot"),
    ("install", "/publishing/install"),
    ("capture", "/update/elapsed"),
    ("measurement", "/reuse/measurement"),
    ("assembly", "/reuse/assembly"),
    ("paint", "/painting/elapsed"),
    ("preparation", "/rendering/preparation"),
    ("submission", "/rendering/submission"),
    ("completion", "/rendering/completion"),
    ("gpu", "/rendering/gpu"),
];
pub const SUMS: &[(&str, &[&str])] = &[
    (
        "construction",
        &[
            "/computation/capture",
            "/computation/measurement",
            "/computation/assembly",
            "/computation/projection",
            "/computation/paint",
            "/computation/scene",
        ],
    ),
    (
        "cpu",
        &["/evaluation", "/publication", "/rendering/preparation"],
    ),
    ("observed", &["/total", "/inspection"]),
];

pub fn sum(stage: &Value, paths: &[&str]) -> Option<u64> {
    paths.iter().try_fold(0_u64, |total, path| {
        let value = if *path == "/rendering/preparation"
            && stage.get("rendering").is_some_and(Value::is_null)
        {
            0
        } else {
            stage.pointer(path)?.as_u64()?
        };
        total.checked_add(value)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Shaping,
    Painting,
    Geometry,
    Images,
    Routing,
}
impl Group {
    pub fn fields(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Shaping => (
                "/reuse/shaping",
                &[
                    "preparations",
                    "shapes",
                    "bytes",
                    "candidates",
                    "materialized",
                ],
            ),
            Self::Painting => (
                "/painting",
                &["requests", "hits", "recorded", "glyphs", "groups"],
            ),
            Self::Geometry => (
                "/rendering/work",
                &[
                    "records", "measured", "excluded", "skipped", "outlines", "decoded", "paths",
                    "culled",
                ],
            ),
            Self::Routing => (
                "",
                &[
                    "reuse/retained",
                    "computation/layouts",
                    "computation/projections",
                    "computation/finalizations",
                    "computation/paints",
                    "computation/checks",
                    "computation/changes",
                    "computation/queries",
                    "publishing/collections",
                    "reuse/isolated",
                    "reuse/deferred",
                    "reuse/dependencies",
                    "routing/checked",
                    "routing/retained",
                    "reuse/shaping/checked",
                    "reuse/shaping/routed",
                    "reuse/construction/text/requests",
                    "reuse/construction/text/retained",
                    "reuse/construction/text/cached",
                    "reuse/construction/text/generated",
                    "painting/checked",
                    "painting/identified",
                    "painting/routed",
                    "painting/built",
                    "rendering/work/undemanded",
                    "rendering/work/subtrees",
                    "rendering/work/bounded",
                    "rendering/work/pruned",
                    "rendering/work/built",
                    "rendering/work/unproven",
                ],
            ),
            Self::Images => (
                "/images",
                &[
                    "scans",
                    "visited",
                    "changes",
                    "candidates",
                    "fetches",
                    "decoded",
                    "bindings",
                ],
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Observation {
    Missing,
    Absent,
    Value(u64),
}
pub fn observe(stage: &Value, path: &str) -> Observation {
    let mut current = stage;
    for part in path.split('/').skip(1) {
        if current.is_null() {
            return Observation::Absent;
        }
        let Some(next) = current.get(part) else {
            return Observation::Missing;
        };
        current = next;
    }
    current
        .as_u64()
        .map_or(Observation::Missing, Observation::Value)
}

#[derive(Debug, Clone, Serialize)]
pub struct Timing {
    pub samples: usize,
    pub minimum: f64,
    pub median: f64,
    pub p95: f64,
    pub maximum: f64,
}
impl Timing {
    pub fn collect(values: Vec<u64>) -> Option<Self> {
        let summary = crate::scenario::Distribution::new(values)?;
        let milliseconds = |value: u64| value as f64 / 1_000_000.0;
        Some(Self {
            samples: summary.samples,
            minimum: milliseconds(summary.minimum),
            median: milliseconds(summary.median),
            p95: milliseconds(summary.p95),
            maximum: milliseconds(summary.maximum),
        })
    }
}
