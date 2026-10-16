pub mod artifact;
pub mod compare;
pub mod paired;
pub mod prerequisite;
pub mod reference;
pub mod summary;

pub type Result<T> = std::result::Result<T, String>;

pub fn digest(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn read(path: &std::path::Path) -> Result<serde_json::Value> {
    let data = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&data).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn write(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    std::fs::write(path, encode(value)?).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn create(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    output
        .write_all(&encode(value)?)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn encode(value: &serde_json::Value) -> Result<Vec<u8>> {
    let mut data = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    data.push(b'\n');
    Ok(data)
}

pub fn repository() -> Result<std::path::PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("repository lookup: {error}"))?;
    if !output.status.success() {
        return Err("repository lookup: not inside a Plectron worktree".into());
    }
    let text = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    Ok(text.trim_end().into())
}

pub fn sources(names: &[&str]) -> Result<serde_json::Value> {
    let root = repository()?.join("tools/chromium/src");
    names
        .iter()
        .map(|name| {
            let path = root.join(name);
            let data =
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            Ok(((*name).to_owned(), serde_json::Value::String(digest(&data))))
        })
        .collect::<Result<serde_json::Map<_, _>>>()
        .map(serde_json::Value::Object)
}
