use crate::{Case, Mode, Phase, Profile, Reason, Result, Selection};
use jsengine::{Engine, Rejection};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Passed,
    Failed,
    Excluded,
    Unsupported,
    Blocked,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub path: String,
    pub status: Status,
    pub reason: Option<Reason>,
    pub diagnostic: Option<String>,
    pub assertions: usize,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub revision: String,
    pub verification: &'static str,
    pub harness: &'static str,
    pub totals: BTreeMap<Status, usize>,
    pub cases: Vec<Outcome>,
}

pub fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/upstream")
}

pub fn verify(file: &str, expected: &str) -> Result<String> {
    if Path::new(file).file_name().and_then(|name| name.to_str()) != Some(file) {
        return Err("upstream filename must be local".into());
    }
    let path = directory().join(file);
    let output = Command::new("git")
        .args(["hash-object", "--no-filters", "--"])
        .arg(&path)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("upstream integrity check failed".into());
    }
    if String::from_utf8_lossy(&output.stdout).trim() != expected {
        return Err(format!("upstream bytes changed: {file}"));
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

pub fn run(profile: &Profile) -> Result<Report> {
    verify(&profile.suite.license, &profile.suite.blob)?;
    Engine::check("const items = [1]; for (let item of items) { item = item + 1; } const object = {value: 1}; object.value = 2;").map_err(|error| error.to_string())?;
    let mut report = Report {
        revision: profile.suite.revision.clone(),
        verification: "selected-strict-parse-and-runtime",
        harness: "native-assert-subset-v2",
        totals: BTreeMap::from([
            (Status::Passed, 0),
            (Status::Failed, 0),
            (Status::Excluded, 0),
            (Status::Unsupported, 0),
            (Status::Blocked, 0),
        ]),
        cases: Vec::new(),
    };
    for case in &profile.cases {
        let source = verify(&case.file, &case.blob)?;
        let (status, diagnostic, assertions) = evaluate(case, &source)?;
        *report.totals.entry(status).or_default() += 1;
        report.cases.push(Outcome {
            path: case.path.clone(),
            status,
            reason: case.reason,
            diagnostic,
            assertions,
        });
    }
    Ok(report)
}

fn evaluate(case: &Case, source: &str) -> Result<(Status, Option<String>, usize)> {
    match case.selection {
        Selection::Excluded => return Ok((Status::Excluded, None, 0)),
        Selection::Unsupported => return Ok((Status::Unsupported, None, 0)),
        Selection::Blocked => return Ok((Status::Blocked, None, 0)),
        Selection::Run => (),
    }
    let metadata = source
        .split_once("/*---")
        .and_then(|(_, rest)| rest.split_once("---*/"))
        .map(|(header, _)| header)
        .ok_or("missing test262 metadata")?;
    if case.mode != Mode::Strict {
        return Err("runner supports the strict profile only".into());
    }
    for flag in ["noStrict", "async", "module", "raw", "includes:"] {
        if metadata
            .lines()
            .any(|line| line.starts_with("flags:") && line.contains(flag))
            || (flag == "includes:" && metadata.lines().any(|line| line.starts_with(flag)))
        {
            return Err("unsupported upstream metadata".into());
        }
    }
    match case.phase {
        Phase::Parse => rejection(source, metadata),
        Phase::Runtime => execution(source, metadata),
    }
}

fn rejection(source: &str, metadata: &str) -> Result<(Status, Option<String>, usize)> {
    for required in ["negative:", "phase: parse", "type: SyntaxError"] {
        if !metadata.contains(required) {
            return Err("unsupported upstream expectation".into());
        }
    }
    Ok(match Engine::check(source) {
        Ok(()) => (Status::Failed, Some("invalid source accepted".into()), 0),
        Err(Rejection::Syntax(error)) => (Status::Passed, Some(error.to_string()), 0),
        Err(Rejection::Fault(error)) => (Status::Failed, Some(error.to_string()), 0),
    })
}

fn execution(source: &str, metadata: &str) -> Result<(Status, Option<String>, usize)> {
    if metadata.lines().any(|line| line.starts_with("negative:")) {
        return Err("runtime negative metadata is not supported".into());
    }
    Ok(match crate::harness::run(source) {
        Ok(assertions) => (Status::Passed, None, assertions),
        Err(diagnostic) => (Status::Failed, Some(diagnostic), 0),
    })
}
