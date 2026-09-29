use crate::{Profile, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize)]
pub struct Budget {
    pub reference: usize,
    pub maintained: usize,
    pub upstream: usize,
    pub groups: BTreeMap<String, usize>,
}

pub fn measure(root: &Path, profile: &Profile) -> Result<Budget> {
    let mut budget = Budget {
        reference: profile.reference,
        ..Budget::default()
    };
    let upstream = profile
        .cases
        .iter()
        .map(|case| Path::new("tools/spec/resources/upstream").join(&case.file))
        .collect();
    budget.visit(root, root, &upstream)?;
    Ok(budget)
}

impl Budget {
    fn visit(&mut self, root: &Path, directory: &Path, upstream: &BTreeSet<PathBuf>) -> Result<()> {
        let entries = std::fs::read_dir(directory).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if !matches!(entry.file_name().to_str(), Some("target" | ".git")) {
                    self.visit(root, &path, upstream)?;
                }
            } else {
                self.record(root, &path, upstream)?;
            }
        }
        Ok(())
    }

    fn record(&mut self, root: &Path, path: &Path, upstream: &BTreeSet<PathBuf>) -> Result<()> {
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("rs" | "grammar" | "js" | "html" | "css" | "json" | "py" | "sh" | "metal")
        ) {
            return Ok(());
        }
        let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
        let lines = source.lines().count();
        if upstream.contains(relative) {
            self.upstream += lines;
            return Ok(());
        }
        self.maintained += lines;
        let parts: Vec<_> = relative.iter().collect();
        let group = if parts[0] == "crates" || parts[0] == "tools" {
            parts[1]
        } else {
            parts[0]
        };
        *self
            .groups
            .entry(group.to_string_lossy().into_owned())
            .or_default() += lines;
        Ok(())
    }
}
