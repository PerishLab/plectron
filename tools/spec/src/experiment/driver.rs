use super::{Comparison, Document, Plan, Summary, plan::Entry, storage};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct Manifest {
    pub version: u32,
    pub plan: Plan,
    pub executable: PathBuf,
    pub fingerprint: u64,
    pub created: u64,
    pub entries: Vec<Result>,
}
impl Manifest {
    pub fn valid(&self) -> bool {
        self.entries.len() == self.plan.entries.len() && self.entries.iter().all(Result::valid)
    }
}
#[derive(Serialize)]
pub struct Result {
    pub id: String,
    pub report: Option<String>,
    pub error: Option<String>,
    pub summary: Option<Summary>,
    pub comparison: Option<Comparison>,
}
pub fn run(
    source: &Path,
    output: &Path,
    platform: (&'static str, &'static str),
    executable: PathBuf,
) -> crate::Result<Manifest> {
    let plan = Plan::read(source)?;
    let fingerprint =
        crate::scenario::fingerprint(&std::fs::read(&executable).map_err(|e| e.to_string())?);
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir(output).map_err(|e| {
        format!(
            "{}: new experiment directory required; choose another --output path: {e}",
            output.display()
        )
    })?;
    let mut manifest = Manifest {
        version: 1,
        plan,
        executable,
        fingerprint,
        created,
        entries: Vec::new(),
    };
    storage::checkpoint(output, &manifest)?;
    for (index, entry) in manifest.plan.entries.iter().enumerate() {
        eprintln!(
            "[{}/{}] {}: {} {} / {} / {} runs ({})",
            index + 1,
            manifest.plan.entries.len(),
            entry.id,
            entry.scenario,
            entry.size,
            entry.backend,
            entry.runs,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        manifest
            .entries
            .push(execute(entry, (source, output), platform));
        storage::checkpoint(output, &manifest)?;
        eprintln!(
            "  {}: {} (saved {})",
            entry.id,
            if manifest.entries.last().is_some_and(Result::valid) {
                "passed"
            } else {
                "failed; inspect report and manifest"
            },
            output.display()
        );
    }
    Ok(manifest)
}
impl Result {
    fn valid(&self) -> bool {
        self.error.is_none()
            && self.summary.as_ref().is_some_and(Summary::valid)
            && self
                .comparison
                .as_ref()
                .is_none_or(|comparison| comparison.passed)
    }
}
fn execute(entry: &Entry, paths: (&Path, &Path), platform: (&'static str, &'static str)) -> Result {
    let mut result = Result {
        id: entry.id.clone(),
        report: None,
        error: None,
        summary: None,
        comparison: None,
    };
    if let Err(error) = observe::call(format!("entry.{}", entry.id), || {
        measure(entry, paths, platform, &mut result)
    }) {
        result.error = Some(error);
    }
    result
}
fn measure(
    entry: &Entry,
    paths: (&Path, &Path),
    platform: (&'static str, &'static str),
    result: &mut Result,
) -> crate::Result<()> {
    let config = config::Store::default();
    for (key, value) in &entry.config {
        config.set(key, value.clone());
    }
    config.set("scenario.runs", entry.runs.into());
    let report = crate::scenario::configured(
        &entry.scenario,
        crate::scenario::Options {
            size: &entry.size,
            mode: "--bench",
            backend: &entry.backend,
            platform,
        },
        config,
    )?;
    let name = format!("{}.json", entry.id);
    storage::File(&paths.1.join(&name)).write(&report)?;
    result.report = Some(name);
    let document = Document::parse(serde_json::to_value(report).map_err(|e| e.to_string())?)?;
    result.summary = Some(document.summary());
    if let Some(baseline) = &entry.baseline {
        let path = paths.0.parent().unwrap_or(Path::new(".")).join(baseline);
        let before = super::read(&path)?;
        result.comparison = Some(super::compare(&before, &document, &entry.rules));
    }
    Ok(())
}
