pub mod bench;
pub mod handoff;
mod live;
pub mod native;
pub mod report;
pub mod snap;
pub mod trace;

pub use live::{activity, application, feel, frames, ipc, passive};

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

pub fn digest(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn read(path: &Path) -> Result<Value> {
    let data = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&data).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn write(path: &Path, value: &Value) -> Result<()> {
    let mut data = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    data.push(b'\n');
    std::fs::write(path, data).map_err(|error| format!("{}: {error}", path.display()))
}

pub struct Output;

impl Output {
    pub fn fresh(path: &Path, value: &Value) -> Result<()> {
        use std::io::Write;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let mut data = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
        data.push(b'\n');
        output
            .write_all(&data)
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn directory(path: &Path) -> Result<()> {
        std::fs::create_dir(path).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn prepare(path: &Path) -> Result<PathBuf> {
        let path = std::path::absolute(path).map_err(|error| error.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        Self::directory(&path)?;
        Ok(path)
    }
}

pub fn absolute(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))
}

pub fn number(value: &Value, label: &str) -> Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| format!("{label} must be numeric"))
}

pub fn integer(value: &Value, label: &str) -> Result<i64> {
    value
        .as_i64()
        .ok_or_else(|| format!("{label} must be an integer"))
}

pub fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} must be an array"))
}
