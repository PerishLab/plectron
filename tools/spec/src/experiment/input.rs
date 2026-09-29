use crate::Result;
use serde_json::Value;
use std::collections::BTreeSet;

pub const IDENTITY: &[&str] = &[
    "/case/id",
    "/case/entry",
    "/case/prefix",
    "/case/region",
    "/case/actions",
    "/case/viewport",
    "/count",
    "/edits",
    "/backend",
    "/architecture",
    "/system",
    "/debug",
    "/unit",
    "/rasterization",
    "/endpoint",
    "/accounting",
];
pub struct Document {
    pub(super) value: Value,
    pub(super) actions: Vec<String>,
    pub(super) issues: Vec<String>,
}
impl Document {
    pub fn parse(value: Value) -> Result<Self> {
        let runs = value
            .get("runs")
            .and_then(Value::as_array)
            .ok_or("scenario report requires runs array")?;
        let declared = value
            .pointer("/case/actions")
            .and_then(Value::as_array)
            .ok_or("scenario report requires case.actions")?;
        let mut actions = vec!["seed".into(), "steady".into()];
        for action in declared {
            actions.push(
                action
                    .as_str()
                    .ok_or("scenario action must be text")?
                    .into(),
            );
        }
        let mut issues = Vec::new();
        if runs.is_empty() {
            issues.push("report has no runs".into());
        }
        if actions.iter().collect::<BTreeSet<_>>().len() != actions.len() {
            issues.push("duplicate declared action".into());
        }
        for path in IDENTITY {
            if value.pointer(path).is_none_or(Value::is_null) {
                issues.push(format!("missing identity {path}"));
            }
        }
        if value.get("unit").and_then(Value::as_str) != Some("nanoseconds") {
            issues.push("unsupported timing unit".into());
        }
        for (index, run) in runs.iter().enumerate() {
            audit(run, &actions, &mut issues, index);
        }
        header(&value, &mut issues);
        match repetitions(&value) {
            Some(expected) if expected == runs.len() as u64 => (),
            Some(_) => issues.push("run count differs from declared repetitions".into()),
            None => issues.push("missing declared repetitions".into()),
        }
        Ok(Self {
            value,
            actions,
            issues,
        })
    }
    pub(super) fn runs(&self) -> &[Value] {
        self.value["runs"].as_array().unwrap()
    }
    pub fn summary(&self) -> super::Summary {
        super::summary::summarize(self)
    }
}
pub(super) fn repetitions(value: &Value) -> Option<u64> {
    value
        .get("repetitions")
        .or_else(|| value.pointer("/runs/0/config/scenario.runs"))?
        .as_u64()
}
fn header(value: &Value, issues: &mut Vec<String>) {
    for path in [
        "/case/id",
        "/case/entry",
        "/case/prefix",
        "/case/region",
        "/backend",
        "/architecture",
        "/system",
        "/rasterization",
        "/endpoint",
        "/accounting",
    ] {
        if value
            .pointer(path)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            issues.push(format!("invalid identity {path}"));
        }
    }
    for field in ["count", "edits"] {
        if value
            .get(field)
            .and_then(Value::as_u64)
            .is_none_or(|v| v == 0)
        {
            issues.push(format!("invalid workload {field}"));
        }
    }
    if value.get("debug").and_then(Value::as_bool).is_none() {
        issues.push("invalid build mode".into());
    }
    let valid = value
        .pointer("/case/viewport")
        .and_then(Value::as_array)
        .is_some_and(|v| v.len() == 2 && v.iter().all(|v| v.as_u64().is_some_and(|n| n > 0)));
    if !valid {
        issues.push("invalid viewport".into());
    }
}
fn audit(run: &Value, actions: &[String], issues: &mut Vec<String>, index: usize) {
    let mut errors = Vec::new();
    if run.get("closed").and_then(Value::as_bool) != Some(true) {
        errors.push("page not closed".into());
    }
    match run.get("failure") {
        Some(Value::Null) => (),
        Some(failure) => errors.push(format!("run failed: {failure}")),
        None => errors.push("missing failure status".into()),
    }
    for field in ["config", "assets"] {
        if !run.get(field).is_some_and(Value::is_object) {
            errors.push(format!("missing {field}"));
        }
    }
    for (url, asset) in run
        .get("assets")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        for field in ["bytes", "fingerprint", "requests"] {
            if asset.get(field).and_then(Value::as_u64).is_none() {
                errors.push(format!("asset {url}: missing {field}"));
            }
        }
    }
    let stages = run.get("stages").and_then(Value::as_array);
    let actual: Vec<_> = stages
        .into_iter()
        .flatten()
        .map(|s| s.get("action").and_then(Value::as_str))
        .collect();
    if !actual
        .iter()
        .copied()
        .eq(actions.iter().map(|action| Some(action.as_str())))
    {
        errors.push("missing, duplicate or reordered stages".into());
    }
    for stage in stages.into_iter().flatten() {
        timing(stage, &mut errors);
    }
    issues.extend(errors.into_iter().map(|e| format!("run {index}: {e}")));
}
fn timing(stage: &Value, errors: &mut Vec<String>) {
    let name = stage.get("action").and_then(Value::as_str).unwrap_or("?");
    let mut fields = vec!["/total", "/evaluation", "/publication"];
    match stage.get("rendering") {
        Some(Value::Null) => (),
        Some(value) if value.is_object() => fields.extend([
            "/rendering/preparation",
            "/rendering/submission",
            "/rendering/completion",
            "/rendering/checksum",
            "/rendering/maximum",
        ]),
        _ => errors.push(format!("{name}: missing or invalid rendering status")),
    }
    for field in fields {
        if stage.pointer(field).and_then(Value::as_u64).is_none() {
            errors.push(format!("{name}: missing or invalid {field}"));
        }
    }
    let sum = [
        "/evaluation",
        "/publication",
        "/rendering/preparation",
        "/rendering/completion",
    ]
    .iter()
    .map(|path| stage.pointer(path).and_then(Value::as_u64).unwrap_or(0))
    .fold(0_u64, u64::saturating_add);
    if stage.get("total").and_then(Value::as_u64) != Some(sum) {
        errors.push(format!("{name}: total differs from accounted intervals"));
    }
}
