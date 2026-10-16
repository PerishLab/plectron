use crate::summary::spread;
use crate::{Result, digest, write};
use base64::Engine;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

pub fn resources(
    root: &Path,
    scenario: &str,
    stylesheet: &str,
    requests: &[String],
) -> Result<Value> {
    let sheet = root.join(scenario).join("style.css");
    let style =
        std::fs::read_to_string(&sheet).map_err(|error| format!("{}: {error}", sheet.display()))?;
    let pattern = Regex::new(r#"url\((?:"([^"]+)"|'([^']+)'|([^)'"]+))\)"#)
        .map_err(|error| error.to_string())?;
    let mut names = BTreeSet::new();
    names.insert(format!("{scenario}/style.css"));
    names.insert("benchmarks/normalize.css".to_owned());
    let scripts = crate::artifact::scripts(root, scenario)?;
    names.extend(scripts.keys().cloned());
    for capture in pattern.captures_iter(&style) {
        let name = (1..=3)
            .find_map(|index| capture.get(index))
            .map(|value| value.as_str())
            .ok_or_else(|| "stylesheet URL has no resource".to_owned())?;
        if !name.contains("://") && !name.starts_with("data:") {
            names.insert(normalize(&format!("{scenario}/{name}"))?);
        }
    }
    for name in requests {
        if name != &format!("{scenario}/index.html") && root.join(name).is_file() {
            names.insert(name.clone());
        }
    }
    let mut result = Map::new();
    for name in names {
        let path = root.join(&name);
        let mut data = match scripts.get(&name) {
            Some(script) => script["body"]
                .as_str()
                .ok_or("compiled script has no body")?
                .as_bytes()
                .to_vec(),
            None => std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?,
        };
        if name.ends_with("/style.css") {
            data.push(b'\n');
            data.extend_from_slice(stylesheet.as_bytes());
        }
        let fingerprint = crate::artifact::fingerprint(&data);
        result.insert(
            name,
            json!({"bytes": data.len(), "fingerprint": fingerprint}),
        );
    }
    Ok(Value::Object(result))
}

pub fn save(output: &Path, report: &mut Value) -> Result<()> {
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let runs = report["runs"]
        .as_array_mut()
        .ok_or_else(|| "runs must be an array".to_owned())?;
    for run in runs {
        let index = run["index"]
            .as_u64()
            .ok_or_else(|| "run index must be an integer".to_owned())?;
        let stages = run["stages"]
            .as_array_mut()
            .ok_or_else(|| "stages must be an array".to_owned())?;
        for stage in stages {
            let action = stage["action"]
                .as_str()
                .ok_or_else(|| "stage action must be a string".to_owned())?
                .to_owned();
            let data = decode(stage, "png")?;
            if !data.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err("capture did not return PNG".into());
            }
            let name = format!("{index}-{action}.png");
            std::fs::write(output.join(&name), &data).map_err(|error| error.to_string())?;
            stage["file"] = json!(name);
            stage["sha256"] = json!(digest(&data));
            let oracle = decode(stage, "oracle")?;
            let expected = format!("{index}-{action}-oracle.png");
            std::fs::write(output.join(&expected), oracle).map_err(|error| error.to_string())?;
            stage["oracle"] = json!(expected);
            groups.entry(action).or_default().push(stage.clone());
        }
    }
    let mut actions = Map::new();
    for (action, stages) in groups {
        let mut summary = Map::new();
        for name in ["ready", "dispatch", "capture"] {
            summary.insert(
                name.to_owned(),
                spread(
                    &stages
                        .iter()
                        .map(|stage| stage[name].clone())
                        .collect::<Vec<_>>(),
                )?,
            );
        }
        let work = stages[0]["work"]
            .as_object()
            .ok_or_else(|| "work must be an object".to_owned())?;
        summary.insert(
            "work".into(),
            Value::Object(
                work.keys()
                    .map(|name| {
                        Ok((
                            name.clone(),
                            spread(
                                &stages
                                    .iter()
                                    .map(|stage| stage["work"][name].clone())
                                    .collect::<Vec<_>>(),
                            )?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            ),
        );
        actions.insert(action, Value::Object(summary));
    }
    write(
        &output.join("summary.json"),
        &json!({
            "unit": "milliseconds",
            "actions": actions,
            "endpoint": report["endpoint"],
            "accounting": report["accounting"],
        }),
    )
}

fn decode(stage: &mut Value, name: &str) -> Result<Vec<u8>> {
    let encoded = stage
        .as_object_mut()
        .and_then(|stage| stage.remove(name))
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| format!("stage has no {name} payload"))?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| error.to_string())
}

pub(crate) fn normalize(path: &str) -> Result<String> {
    let mut held = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(name) => held.push(name),
            Component::CurDir => {}
            Component::ParentDir if !held.pop() => {
                return Err(format!("resource escapes application root: {path}"));
            }
            Component::ParentDir => {}
            _ => return Err(format!("resource is not relative: {path}")),
        }
    }
    Ok(held.to_string_lossy().replace('\\', "/"))
}
