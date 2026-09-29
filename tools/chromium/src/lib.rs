pub mod compare;
pub mod paired;
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

pub fn write_new(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
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
