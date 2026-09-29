use crate::Result;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet as Set};
use std::path::Path;
use std::process::Command;

#[derive(Default)]
pub struct Graph {
    paths: BTreeMap<String, String>,
    reverse: BTreeMap<String, Set<String>>,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    #[serde(rename = "manifest_path")]
    manifest: String,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    kind: Option<String>,
}

impl Graph {
    pub fn load(root: &Path) -> Result<Self> {
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .current_dir(root)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        let metadata: Metadata =
            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        let mut graph = Self::default();
        for package in &metadata.packages {
            let manifest = Path::new(&package.manifest);
            let directory = manifest.parent().ok_or("package manifest has no parent")?;
            let relative = directory.strip_prefix(root).map_err(|e| e.to_string())?;
            graph.paths.insert(
                relative.to_string_lossy().replace('\\', "/"),
                package.name.clone(),
            );
        }
        for package in metadata.packages {
            for dependency in package.dependencies {
                if dependency
                    .kind
                    .as_deref()
                    .is_none_or(|kind| kind == "normal")
                {
                    graph
                        .reverse
                        .entry(dependency.name)
                        .or_default()
                        .insert(package.name.clone());
                }
            }
        }
        Ok(graph)
    }

    pub fn package(&self, path: &Path) -> Option<&str> {
        let path = path.to_string_lossy();
        self.paths
            .iter()
            .filter(|(root, _)| path == root.as_str() || path.starts_with(&format!("{root}/")))
            .max_by_key(|(root, _)| root.len())
            .map(|(_, package)| package.as_str())
    }

    pub fn consumers(&self, package: &str) -> impl Iterator<Item = &str> {
        self.reverse
            .get(package)
            .into_iter()
            .flatten()
            .map(String::as_str)
    }
}
