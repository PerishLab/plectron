use crate::{Result, create};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Output;

pub fn record(root: &Path, result: Result<Output>) -> Result<Value> {
    let mut receipt = json!({
        "argv": ["playwright-cli", "--version"],
        "stdout": [], "stderr": [], "exit": null,
        "version": null, "error": null,
    });
    let version = match result {
        Ok(output) => {
            receipt["stdout"] = json!(&output.stdout);
            receipt["stderr"] = json!(&output.stderr);
            receipt["exit"] = json!(output.status.code());
            if output.status.success() {
                version(&output.stdout)
            } else {
                Err(format!(
                    "playwright-cli --version failed: {}",
                    output.status
                ))
            }
        }
        Err(error) => Err(format!("required playwright-cli is unavailable: {error}")),
    };
    match &version {
        Ok(version) => receipt["version"] = json!(version),
        Err(error) => receipt["error"] = json!(error),
    }
    create(&root.join("prerequisites.json"), &receipt)?;
    version.map(|_| receipt)
}

fn version(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "playwright-cli version is not UTF-8".to_string())?
        .trim();
    if text.is_empty() {
        return Err("playwright-cli version is empty".into());
    }
    Ok(text.into())
}
