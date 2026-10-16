use super::check::text;
use crate::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct Stage {
    pub file: PathBuf,
    pub geometry: Value,
    pub records: Value,
    pub ready: Value,
    pub work: Value,
}

impl Stage {
    pub fn native(report: &Path, stage: &Value) -> Result<Self> {
        let capture = &stage["capture"];
        let named = text(&capture["file"], "native capture artifact")?;
        let mut file = report
            .parent()
            .unwrap_or(Path::new("."))
            .join(Path::new(named).file_name().unwrap_or_default());
        if !file.exists() {
            file = crate::repository()?.join(named);
        }
        let rendering = stage["rendering"].as_object();
        let nanos = |name: &str| {
            rendering
                .and_then(|values| values.get(name))
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                / 1e6
        };
        let content = (stage["evaluation"].as_f64().unwrap_or(0.0)
            + stage["publication"].as_f64().unwrap_or(0.0))
            / 1e6;
        let preparation = nanos("preparation");
        Ok(Self {
            file,
            geometry: stage["geometry"].clone(),
            records: stage["records"].clone(),
            ready: json!(
                capture["ready"]
                    .as_f64()
                    .ok_or("native ready must be numeric")?
                    / 1e6
            ),
            work: json!({
                "preparation": preparation,
                "completion": nanos("completion"),
                "readback": nanos("readback"),
                "content": content,
                "encoding": capture["encoding"].as_f64().unwrap_or(0.0) / 1e6,
                "total": stage["total"].as_f64().unwrap_or(0.0) / 1e6,
                "cpu": content + preparation,
            }),
        })
    }

    pub fn browser(directory: &Path, stage: &Value) -> Result<Self> {
        Ok(Self {
            file: directory.join(text(&stage["file"], "browser capture")?),
            geometry: stage["state"]["geometry"].clone(),
            records: stage["state"]["records"].clone(),
            ready: stage["ready"].clone(),
            work: stage.get("work").cloned().unwrap_or_else(|| json!({})),
        })
    }

    pub fn summary(&self) -> Value {
        json!({"geometry": self.geometry, "records": self.records, "ready": self.ready, "work": self.work})
    }
}
