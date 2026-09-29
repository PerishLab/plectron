use crate::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub kind: Kind,
    pub path: PathBuf,
    pub previous: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Staged,
    Worktree,
    Range(String),
    File(PathBuf),
}

impl Source {
    pub fn read(&self, root: &Path) -> Result<Vec<Change>> {
        let text = match self {
            Self::File(path) => std::fs::read_to_string(path).map_err(error)?,
            Self::Staged => git(root, &["diff", "--cached", "--name-status"])?,
            Self::Worktree => git(root, &["diff", "--name-status"])?,
            Self::Range(range) => git(root, &["diff", "--name-status", range])?,
        };
        parse(&text)
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(error)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    String::from_utf8(output.stdout).map_err(error)
}

pub fn parse(text: &str) -> Result<Vec<Change>> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            let status = fields.first().copied().unwrap_or("");
            let kind = match status.chars().next() {
                Some('A') => Kind::Added,
                Some('M' | 'T') => Kind::Modified,
                Some('D') => Kind::Deleted,
                Some('R') => Kind::Renamed,
                Some('C') => Kind::Copied,
                _ => return Err(format!("unsupported change status: {status}")),
            };
            let (previous, path) = if matches!(kind, Kind::Renamed | Kind::Copied) {
                let [_, previous, path] = fields.as_slice() else {
                    return Err(format!("invalid rename record: {line}"));
                };
                (Some(PathBuf::from(previous)), PathBuf::from(path))
            } else {
                let [_, path] = fields.as_slice() else {
                    return Err(format!("invalid change record: {line}"));
                };
                (None, PathBuf::from(path))
            };
            Ok(Change {
                kind,
                path,
                previous,
            })
        })
        .collect()
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
