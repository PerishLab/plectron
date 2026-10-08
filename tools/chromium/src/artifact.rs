use crate::{Result, digest};
use base64::Engine;
use project::{Adapters, Kind, Target, Workspace};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub fn compile(path: &Path) -> Result<Value> {
    let mut workspace = Workspace::open(path, Adapters::new(typescript::Typescript, pt::Templates))
        .map_err(|error| error.to_string())?;
    let output = workspace
        .link(Target::Script)
        .map_err(|error| error.to_string())?;
    let mut sources = Map::new();
    for (module, node) in &output.graph.modules {
        let bytes = std::fs::read(workspace.root().join(module.path()))
            .map_err(|error| error.to_string())?;
        let kind = match node.kind {
            Kind::Js => "js",
            Kind::Ts => "ts",
            Kind::Pt => "pt",
        };
        sources.insert(
            module.path().into(),
            json!({"kind": kind, "sha256": digest(&bytes)}),
        );
    }
    Ok(json!({
        "body": output.javascript,
        "sources": sources,
        "origins": output.origins.iter().map(|origin| json!({
            "module": origin.module.path(),
            "generated": [origin.generated.start, origin.generated.end],
            "source": [origin.source.start, origin.source.end],
            "mapping": format!("{:?}", origin.mapping),
        })).collect::<Vec<_>>(),
    }))
}

pub fn scripts(root: &Path, scenario: &str) -> Result<BTreeMap<String, Value>> {
    entries(root, scenario)?
        .into_iter()
        .map(|path| Ok((path.clone(), compile(&root.join(path))?)))
        .collect()
}

pub fn entries(root: &Path, scenario: &str) -> Result<Vec<String>> {
    let html = std::fs::read_to_string(root.join(scenario).join("index.html"))
        .map_err(|error| error.to_string())?;
    let pattern =
        Regex::new(r#"<script\s+[^>]*src=["']([^"']+)["']"#).map_err(|error| error.to_string())?;
    let mut entries = Vec::new();
    for capture in pattern.captures_iter(&html) {
        let path = crate::paired::normalize(&format!("{scenario}/{}", &capture[1]))?;
        if entries.contains(&path) {
            return Err(format!("duplicate browser script entry: {path}"));
        }
        entries.push(path);
    }
    if entries.is_empty() {
        return Err("browser workload has no explicit script entries".into());
    }
    Ok(entries)
}

pub fn verify(scripts: &BTreeMap<String, Value>, receipts: &Value) -> Result<Value> {
    let receipts = receipts
        .as_array()
        .ok_or("browser script receipts must be an array")?;
    let mut observed = Map::new();
    for receipt in receipts {
        let name = receipt["path"]
            .as_str()
            .ok_or("script receipt has no path")?;
        let expected = scripts
            .get(name)
            .ok_or("undeclared browser script response")?;
        let encoded = receipt["body"]
            .as_str()
            .ok_or("script receipt has no body")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| error.to_string())?;
        if receipt["status"] != 200
            || receipt["mime"] != "text/javascript; charset=utf-8"
            || Some(bytes.as_slice()) != expected["body"].as_str().map(str::as_bytes)
        {
            return Err(format!("browser script artifact mismatch: {name}"));
        }
        observed.insert(
            name.into(),
            json!({
                "bytes": bytes.len(), "fingerprint": fingerprint(&bytes), "sha256": digest(&bytes),
                "sources": expected["sources"], "origins": expected["origins"],
            }),
        );
    }
    if observed.len() != scripts.len() {
        return Err("browser did not consume every declared script artifact".into());
    }
    Ok(Value::Object(observed))
}

pub fn fingerprint(data: &[u8]) -> u64 {
    data.iter()
        .fold(14_695_981_039_346_656_037_u64, |held, byte| {
            held.wrapping_mul(1_099_511_628_211) ^ u64::from(*byte)
        })
}
