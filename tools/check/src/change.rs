use crate::Result;
use serde::Serialize;
use std::ffi::OsString;
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

impl Kind {
    fn new(status: &str) -> Result<Self> {
        match status.chars().next() {
            Some('A') => Ok(Self::Added),
            Some('M' | 'T') => Ok(Self::Modified),
            Some('D') => Ok(Self::Deleted),
            Some('R') => Ok(Self::Renamed),
            Some('C') => Ok(Self::Copied),
            Some(_) => Err(format!("unsupported change status: {status}")),
            None => Err("empty change status".into()),
        }
    }

    fn paired(self) -> bool {
        matches!(self, Self::Renamed | Self::Copied)
    }
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
        let bytes = match self {
            Self::File(path) => {
                let bytes = std::fs::read(path).map_err(error)?;
                if !bytes.contains(&0) {
                    return parse(&String::from_utf8(bytes).map_err(error)?);
                }
                bytes
            }
            Self::Staged => git(root, &["diff", "--cached", "--name-status", "-z"])?,
            Self::Worktree => git(root, &["diff", "--name-status", "-z"])?,
            Self::Range(range) => git(root, &["diff", "--name-status", "-z", range])?,
        };
        records(&bytes)
    }
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(error)?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed ({}): {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn records(bytes: &[u8]) -> Result<Vec<Change>> {
    let Some(body) = bytes.strip_suffix(&[0]) else {
        return if bytes.is_empty() {
            Ok(Vec::new())
        } else {
            Err("incomplete change record: missing NUL terminator".into())
        };
    };
    let mut fields = body.split(|byte| *byte == 0);
    let mut changes = Vec::new();
    while let Some(status) = fields.next() {
        let status = std::str::from_utf8(status).map_err(error)?;
        let kind = Kind::new(status)?;
        let mut next = || {
            fields
                .next()
                .filter(|field| !field.is_empty())
                .ok_or_else(|| format!("incomplete change record for status {status}"))
                .and_then(path)
        };
        let first = next()?;
        let (previous, path) = if kind.paired() {
            (Some(first), next()?)
        } else {
            (None, first)
        };
        changes.push(Change {
            kind,
            path,
            previous,
        });
    }
    Ok(changes)
}

pub fn parse(text: &str) -> Result<Vec<Change>> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            let status = fields.first().copied().unwrap_or("");
            let kind = Kind::new(status)?;
            let (previous, path) = if kind.paired() {
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

#[cfg(unix)]
fn path(bytes: &[u8]) -> Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
fn path(bytes: &[u8]) -> Result<PathBuf> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        format!(
            "Git path is not UTF-8: {}",
            String::from_utf8_lossy(bytes).escape_debug()
        )
    })?;
    Ok(PathBuf::from(OsString::from(text)))
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
