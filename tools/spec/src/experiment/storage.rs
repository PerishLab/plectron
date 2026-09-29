use super::{Document, Summary};
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use std::io::{BufReader, BufWriter, Write};
use std::{collections::BTreeMap, path::Path};

pub(super) struct File<'a>(pub &'a Path);
impl File<'_> {
    pub fn read(&self) -> Result<Value> {
        let file = std::fs::File::open(self.0).map_err(|e| format!("{}: {e}", self.0.display()))?;
        serde_json::from_reader(BufReader::new(file))
            .map_err(|e| format!("{}: {e}", self.0.display()))
    }
    pub fn write(&self, value: &impl Serialize) -> Result<()> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.0)
            .map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, value).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())
    }
}
pub(super) fn checkpoint(root: &Path, value: &impl Serialize) -> Result<()> {
    let pending = root.join("manifest.pending");
    File(&pending).write(value)?;
    std::fs::rename(pending, root.join("manifest.json")).map_err(|e| e.to_string())
}
#[derive(Serialize)]
pub struct Inspection {
    pub issues: Vec<String>,
    pub reports: BTreeMap<String, Summary>,
}
impl Inspection {
    pub fn valid(&self) -> bool {
        self.issues.is_empty() && self.reports.values().all(Summary::valid)
    }
}
pub fn inspect(path: &Path) -> Inspection {
    let mut result = Inspection {
        issues: Vec::new(),
        reports: BTreeMap::new(),
    };
    if path.is_dir() {
        if let Err(error) = directory(path, &mut result) {
            result.issues.push(error);
        }
    } else {
        document(path, path.display().to_string(), &mut result);
    }
    result
}
fn document(path: &Path, name: String, result: &mut Inspection) -> Option<Document> {
    match File(path).read().and_then(Document::parse) {
        Ok(report) => {
            result.reports.insert(name, report.summary());
            Some(report)
        }
        Err(error) => {
            result.issues.push(error);
            None
        }
    }
}
fn directory(root: &Path, result: &mut Inspection) -> Result<()> {
    let manifest = File(&root.join("manifest.json")).read()?;
    if manifest.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("unsupported experiment manifest".into());
    }
    let plan: super::Plan =
        serde_json::from_value(manifest["plan"].clone()).map_err(|e| e.to_string())?;
    plan.validate()?;
    let entries = manifest
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("missing manifest entries")?;
    if entries.len() != plan.entries.len() {
        result.issues.push("experiment is incomplete".into());
    }
    for entry in &plan.entries {
        let records: Vec<_> = entries
            .iter()
            .filter(|record| record["id"].as_str() == Some(&entry.id))
            .collect();
        if records.len() != 1 {
            result
                .issues
                .push(format!("{}: missing or duplicate result", entry.id));
            continue;
        }
        record(root, records[0], entry, result)?;
    }
    Ok(())
}
fn record(
    root: &Path,
    entry: &Value,
    planned: &super::plan::Entry,
    result: &mut Inspection,
) -> Result<()> {
    let name = entry["id"].as_str().ok_or("invalid result identity")?;
    if planned.baseline.is_some() && !entry.get("comparison").is_some_and(Value::is_object) {
        result
            .issues
            .push(format!("{name}: missing required comparison"));
    }
    if entry.get("error") != Some(&Value::Null) {
        result.issues.push(format!("{name}: {}", entry["error"]));
    }
    if entry
        .get("comparison")
        .is_some_and(|value| !value.is_null())
        && entry["comparison"]["passed"] != true
    {
        result
            .issues
            .push(format!("{name}: baseline comparison did not pass"));
    }
    if entry["report"].as_str() != Some(&format!("{name}.json")) {
        result.issues.push(format!("{name}: missing report"));
    } else {
        if let Some(report) = document(&root.join(format!("{name}.json")), name.into(), result) {
            verify(&report, planned, &mut result.issues);
        }
    }
    Ok(())
}
fn verify(report: &Document, planned: &super::plan::Entry, issues: &mut Vec<String>) {
    for (path, expected) in [
        ("/case/id", &planned.scenario),
        ("/size", &planned.size),
        ("/backend", &planned.backend),
    ] {
        if report.value.pointer(path).and_then(Value::as_str) != Some(expected) {
            issues.push(format!("{}: report disagrees with plan {path}", planned.id));
        }
    }
    let mut config = planned.config.clone();
    config.insert("scenario.runs".into(), planned.runs.into());
    let expected = serde_json::to_value(config).expect("JSON entries");
    if report.runs().len() != planned.runs
        || report
            .runs()
            .iter()
            .any(|r| r.get("config") != Some(&expected))
    {
        issues.push(format!(
            "{}: report disagrees with planned repetitions or configuration",
            planned.id
        ));
    }
}
