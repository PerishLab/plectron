pub mod dispatch;
pub mod font;
pub mod oracle;
pub mod startup;
pub mod stream;

use crate::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

fn output(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::create_dir(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

fn write(path: &Path, value: &Value) -> Result<()> {
    let mut data = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    data.push(b'\n');
    std::fs::write(path, data).map_err(|error| format!("{}: {error}", path.display()))
}

fn digest(path: &Path) -> Result<String> {
    let data = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn platform() -> String {
    Command::new("uname")
        .args(["-s", "-r", "-m"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| format!("{} {}", std::env::consts::ARCH, std::env::consts::OS))
}

fn execute(binary: &Path, arguments: &[String]) -> Result<Value> {
    let output = Command::new(binary)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if error.is_empty() {
            format!("command exited {}", output.status)
        } else {
            error
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
}

fn stats(values: &[Value]) -> Result<Value> {
    if values.is_empty() {
        return Err("distribution has no samples".into());
    }
    let mut values = values.to_vec();
    values.sort_by(|left, right| {
        left.as_f64()
            .unwrap_or(f64::NAN)
            .total_cmp(&right.as_f64().unwrap_or(f64::NAN))
    });
    if values.iter().any(|value| value.as_f64().is_none()) {
        return Err("distribution contains a nonnumeric sample".into());
    }
    let middle = values.len() / 2;
    let median = if values.len().is_multiple_of(2) {
        json!((values[middle - 1].as_f64().unwrap() + values[middle].as_f64().unwrap()) / 2.0)
    } else {
        values[middle].clone()
    };
    Ok(json!({"minimum": values[0], "median": median, "maximum": values[values.len() - 1]}))
}

struct Arguments<'a> {
    values: &'a [String],
}

impl<'a> Arguments<'a> {
    fn new(values: &'a [String]) -> Self {
        Self { values }
    }

    fn one(&self, name: &str) -> Option<&'a str> {
        self.values
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].as_str())
    }

    fn many(&self, name: &str) -> Vec<&'a str> {
        let Some(index) = self.values.iter().position(|value| value == name) else {
            return Vec::new();
        };
        self.values[index + 1..]
            .iter()
            .take_while(|value| !value.starts_with("--"))
            .map(String::as_str)
            .collect()
    }

    fn reject(&self, allowed: &[&str]) -> Result<()> {
        for name in self.values.iter().filter(|value| value.starts_with("--")) {
            if !allowed.contains(&name.as_str()) {
                return Err(format!("unexpected argument: {name}"));
            }
        }
        Ok(())
    }
}
