use crate::{Artifact, Fault};
use std::path::{Path, PathBuf};

pub fn build(entry: &Path) -> Result<Artifact, Fault> {
    link(entry, false)
}

pub fn script(entry: &Path) -> Result<Artifact, Fault> {
    link(entry, true)
}

fn link(entry: &Path, script: bool) -> Result<Artifact, Fault> {
    let entry = entry.canonicalize().map_err(error)?;
    let root = entry
        .ancestors()
        .find(|path| path.join("pnpm-workspace.yaml").is_file())
        .ok_or_else(|| Fault("build entry is outside a pnpm workspace".into()))?;
    let entry = entry
        .to_str()
        .ok_or_else(|| Fault("entry path is not UTF-8".into()))?;
    if script {
        crate::script(
            entry,
            |path| std::fs::read_to_string(path).map_err(error),
            |from, specifier| resolve(root, from, specifier),
        )
    } else {
        crate::bundle(
            entry,
            |path| std::fs::read_to_string(path).map_err(error),
            |from, specifier| resolve(root, from, specifier),
        )
    }
}

fn resolve(root: &Path, from: &str, specifier: &str) -> Result<String, Fault> {
    let path = if specifier.starts_with('.') {
        Path::new(from)
            .parent()
            .expect("module has parent")
            .join(specifier)
    } else if let Some(package) = specifier.strip_prefix("@plectron/") {
        let (name, subpath) = package.split_once('/').unwrap_or((package, ""));
        let directory = root.join("packages").join(name);
        let manifest = std::fs::read_to_string(directory.join("package.json")).map_err(error)?;
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest).map_err(|failure| Fault(failure.to_string()))?;
        let exports = &manifest["exports"];
        let target = if subpath.is_empty() {
            exports.as_str().or_else(|| exports["."].as_str())
        } else {
            exports[format!("./{subpath}")].as_str()
        }
        .ok_or_else(|| Fault(format!("unknown package export: {specifier}")))?;
        directory.join(target)
    } else {
        return Err(Fault(format!("unsupported module specifier: {specifier}")));
    };
    canonical(path)
}

fn canonical(path: PathBuf) -> Result<String, Fault> {
    path.canonicalize()
        .map_err(error)?
        .to_str()
        .map(String::from)
        .ok_or_else(|| Fault("module path is not UTF-8".into()))
}

fn error(failure: std::io::Error) -> Fault {
    Fault(failure.to_string())
}
