use crate::summary::spread;
use crate::{Result, digest, read};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

const EXTENT: (u32, u32) = (720, 640);

pub fn pixels(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNG buffer size is unknown")?
    ];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    if (info.width, info.height) != EXTENT {
        return Err("capture extent mismatch".into());
    }
    let source = &buffer[..info.buffer_size()];
    let mut rgb = Vec::with_capacity(EXTENT.0 as usize * EXTENT.1 as usize * 3);
    match info.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => source
            .chunks_exact(4)
            .for_each(|pixel| rgb.extend_from_slice(&pixel[..3])),
        png::ColorType::Grayscale => source
            .iter()
            .for_each(|value| rgb.extend_from_slice(&[*value; 3])),
        png::ColorType::GrayscaleAlpha => source
            .chunks_exact(2)
            .for_each(|pixel| rgb.extend_from_slice(&[pixel[0]; 3])),
        png::ColorType::Indexed => return Err("indexed PNG was not expanded".into()),
    }
    Ok(digest(&rgb))
}

pub fn difference(before: &Value, after: &Value, path: &str) -> Vec<Value> {
    if before == after {
        return Vec::new();
    }
    if let (Some(left), Some(right)) = (before.as_object(), after.as_object()) {
        let keys: BTreeSet<_> = left.keys().chain(right.keys()).collect();
        return keys
            .into_iter()
            .flat_map(|key| {
                difference(
                    left.get(key).unwrap_or(&Value::Null),
                    right.get(key).unwrap_or(&Value::Null),
                    &format!("{path}/{key}"),
                )
            })
            .collect();
    }
    vec![json!({"path": path, "plectron": before, "chromium": after})]
}

pub fn projection(scenario: &str, value: &Value) -> Value {
    if !matches!(scenario, "article" | "gallery") {
        return value.clone();
    }
    let gallery = scenario == "gallery";
    let boxes = value["boxes"].as_object().cloned().unwrap_or_default();
    let shape = |bounds: &Value| {
        let values = bounds.as_array().cloned().unwrap_or_default();
        Value::Array(
            [0, 2, 3]
                .into_iter()
                .map(|index| round_if(values.get(index).cloned().unwrap_or(Value::Null), gallery))
                .collect(),
        )
    };
    let records: Map<_, _> = boxes
        .iter()
        .filter(|(name, _)| name.starts_with("record-"))
        .map(|(name, bounds)| (name.clone(), shape(bounds)))
        .collect();
    let region = boxes.get("region").cloned().unwrap_or(Value::Null);
    json!({
        "region": region.as_array().map(|items| Value::Array(items.iter().cloned().map(|item| round_if(item, gallery)).collect())).unwrap_or(region),
        "records": records,
        "scroll": value["scroll"],
    })
}

pub fn validate(report: &Value, browser: &Value) -> Result<()> {
    if browser["errors"]
        .as_array()
        .is_none_or(|errors| !errors.is_empty())
    {
        return Err("browser execution failed".into());
    }
    if report["debug"].as_bool() != Some(false) {
        return Err("paired timing requires a release build".into());
    }
    if report["case"]["id"] != browser["config"]["scenario"]
        || report["count"] != browser["config"]["count"]
    {
        return Err("scenario workload mismatch".into());
    }
    let scenario = text(&report["case"]["id"], "scenario id")?;
    for name in [
        format!("{scenario}/style.css"),
        "benchmarks/normalize.css".into(),
    ] {
        if browser["resources"].get(&name).is_none() {
            return Err("incomplete resource identities".into());
        }
    }
    if report["case"]["viewport"] != json!([720, 640]) || report["edits"] != 1 {
        return Err("unsupported viewport or edit workload".into());
    }
    let repetitions = integer(&report["repetitions"], "native repetitions")?;
    let browser_runs = integer(&browser["config"]["runs"], "browser runs")?;
    if repetitions == 0 || browser_runs == 0 {
        return Err("empty repetitions".into());
    }
    let native = array(&report["runs"], "native runs")?;
    let chromium = array(&browser["runs"], "browser runs")?;
    if native.len() != repetitions as usize || chromium.len() != browser_runs as usize {
        return Err("incomplete repetitions".into());
    }
    let mut expected = vec!["seed", "steady"];
    expected.extend(
        array(&report["case"]["actions"], "case actions")?
            .iter()
            .filter_map(Value::as_str),
    );
    for run in native {
        if !run["failure"].is_null() || run["closed"].as_bool() != Some(true) {
            return Err("native scenario failed or retained lifecycle".into());
        }
        if ["scenario.capture", "scenario.normalized"]
            .into_iter()
            .any(|key| run["config"][key].as_bool() != Some(true))
        {
            return Err("native profile must enable normalized capture".into());
        }
        validate_actions(run, &expected, "native")?;
        let resources = browser["resources"]
            .as_object()
            .ok_or_else(|| "browser resources must be an object".to_owned())?;
        for (name, value) in resources {
            let actual = &run["assets"][format!("app://scenarios/{name}")];
            if ["bytes", "fingerprint"]
                .into_iter()
                .any(|key| actual[key] != value[key])
            {
                return Err(format!("resource mismatch: {name}"));
            }
        }
    }
    let mut indices: Vec<_> = chromium
        .iter()
        .filter_map(|run| run["index"].as_u64())
        .collect();
    indices.sort_unstable();
    if indices != (0..browser_runs).collect::<Vec<_>>() {
        return Err("duplicate or missing browser repetition".into());
    }
    for run in chromium {
        validate_actions(run, &expected, "browser")?;
    }
    Ok(())
}

pub fn compare(native: &Path, directory: &Path) -> Result<Value> {
    compare_with(native, directory, pixels)
}

pub fn compare_with<F>(native: &Path, directory: &Path, pixel_digest: F) -> Result<Value>
where
    F: Fn(&Path) -> Result<String>,
{
    let report = read(native)?;
    let browser_path = directory.join("report.json");
    let browser = read(&browser_path)?;
    validate(&report, &browser)?;
    let mut groups: BTreeMap<String, BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    let mut issues = Vec::new();
    for (engine, runs) in [
        ("plectron", &report["runs"]),
        ("chromium", &browser["runs"]),
    ] {
        let mut known: BTreeMap<String, Value> = BTreeMap::new();
        for run in array(runs, "runs")? {
            let mut previous: Option<String> = None;
            for stage in array(&run["stages"], "stages")? {
                let action = text(&stage["action"], "action")?.to_owned();
                let (file, geometry, records, ready, work) = if engine == "plectron" {
                    native_stage(native, stage)?
                } else {
                    browser_stage(directory, stage)?
                };
                let digest = pixel_digest(&file)?;
                if engine == "chromium" {
                    let oracle = directory.join(text(&stage["oracle"], "oracle capture")?);
                    if digest != pixel_digest(&oracle)? {
                        issue(
                            &mut issues,
                            engine,
                            &action,
                            "capture differs from settled oracle",
                        );
                    }
                }
                if action == "steady" && previous.as_ref() != Some(&digest) {
                    issue(
                        &mut issues,
                        engine,
                        &action,
                        "unchanged action changed pixels",
                    );
                }
                if previous.as_ref() == Some(&digest)
                    && !matches!(action.as_str(), "steady" | "seed")
                {
                    issue(
                        &mut issues,
                        engine,
                        &action,
                        "mutation did not change captured pixels",
                    );
                }
                let observed = json!([geometry, records, digest]);
                if known.get(&action).is_some_and(|held| held != &observed) {
                    issue(
                        &mut issues,
                        engine,
                        &action,
                        "workload or pixels changed between runs",
                    );
                }
                known.insert(action.clone(), observed);
                previous = Some(digest);
                groups
                    .entry(engine.into())
                    .or_default()
                    .entry(action)
                    .or_default()
                    .push(json!({"geometry": geometry, "records": records, "ready": ready, "work": work}));
            }
        }
    }
    let scenario = text(&report["case"]["id"], "scenario id")?;
    let native_groups = groups
        .get("plectron")
        .ok_or("native action groups are absent")?;
    let browser_groups = groups
        .get("chromium")
        .ok_or("browser action groups are absent")?;
    let mut actions = Map::new();
    for (action, values) in native_groups {
        let other = browser_groups
            .get(action)
            .ok_or_else(|| format!("browser action is absent: {action}"))?;
        let before = json!({"geometry": projection(scenario, &values[0]["geometry"]), "records": values[0]["records"]});
        let after = json!({"geometry": projection(scenario, &other[0]["geometry"]), "records": other[0]["records"]});
        let differences = difference(&before, &after, "");
        let variations = difference(&values[0]["geometry"], &other[0]["geometry"], "");
        let mut work = Map::new();
        for (engine, stages) in [("plectron", values), ("chromium", other)] {
            let names = stages[0]["work"]
                .as_object()
                .ok_or("stage work must be an object")?;
            let distributions: Map<_, _> = names
                .keys()
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
                .collect::<Result<_>>()?;
            work.insert(engine.into(), Value::Object(distributions));
        }
        actions.insert(action.clone(), json!({
            "geometry": differences.is_empty(),
            "differences": differences,
            "variations": variations,
            "plectron": spread(&values.iter().map(|value| value["ready"].clone()).collect::<Vec<_>>())?,
            "chromium": spread(&other.iter().map(|value| value["ready"].clone()).collect::<Vec<_>>())?,
            "work": work,
        }));
    }
    let mut limits = vec![
        "Native invocation is in-process; Chromium includes CDP command transport and capture scheduling.",
        "PNG compression and renderer antialiasing differ; matching selected geometry is not pixel equivalence.",
        "Chromium Date.now has millisecond resolution; this does not measure physical presentation or pure engine execution.",
    ];
    if matches!(scenario, "article" | "gallery") {
        limits.push("Flow equivalence compares content geometry and scroll state; vertical offsets and extent retain Plectron's non-collapsing margin model as reported variations. Gallery content edges are compared at Plectron's integer-pixel geometry boundary.");
    }
    let geometry_passed = actions
        .values()
        .all(|action| action["geometry"].as_bool() == Some(true));
    let native_bytes = std::fs::read(native).map_err(|error| error.to_string())?;
    let browser_bytes = std::fs::read(&browser_path).map_err(|error| error.to_string())?;
    Ok(json!({
        "schema": 1,
        "status": "provisional",
        "passed": issues.is_empty() && geometry_passed,
        "analysis_sources": {
            "compare.rs": digest(include_bytes!("compare.rs")),
            "summary.rs": digest(include_bytes!("summary.rs")),
            "paired.rs": digest(include_bytes!("paired.rs")),
        },
        "classification": "directional capture API observation",
        "unit": "milliseconds",
        "actions": actions,
        "issues": issues,
        "endpoints": {"plectron": report["capture"], "chromium": browser["endpoint"]},
        "evidence": {"plectron": digest(&native_bytes), "chromium": digest(&browser_bytes)},
        "identity": {
            "scenario": scenario,
            "count": report["count"],
            "native": select(&report, &["backend", "architecture", "system", "rasterization", "debug"]),
            "config": report["runs"][0]["config"].as_object().map(|values| Value::Object(values.iter().filter(|(key, _)| key.as_str() != "scenario.output").map(|(key, value)| (key.clone(), value.clone())).collect())).unwrap_or_default(),
            "browser": select(&browser, &["version", "host", "sources", "adaptations", "accounting", "prerequisites"]),
            "sampling": select(&browser["config"], &["runs", "warmups"]),
        },
        "ratio": Value::Null,
        "limits": limits,
    }))
}

fn native_stage(native: &Path, stage: &Value) -> Result<(PathBuf, Value, Value, Value, Value)> {
    let capture = &stage["capture"];
    let named = text(&capture["file"], "native capture artifact")?;
    let mut file = native
        .parent()
        .unwrap_or(Path::new("."))
        .join(Path::new(named).file_name().unwrap_or_default());
    if !file.exists() {
        file = repository_root().join(named);
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
    Ok((
        file,
        stage["geometry"].clone(),
        stage["records"].clone(),
        json!(
            capture["ready"]
                .as_f64()
                .ok_or("native ready must be numeric")?
                / 1e6
        ),
        json!({
            "preparation": preparation,
            "completion": nanos("completion"),
            "readback": nanos("readback"),
            "content": content,
            "encoding": capture["encoding"].as_f64().unwrap_or(0.0) / 1e6,
            "total": stage["total"].as_f64().unwrap_or(0.0) / 1e6,
            "cpu": content + preparation,
        }),
    ))
}

fn browser_stage(directory: &Path, stage: &Value) -> Result<(PathBuf, Value, Value, Value, Value)> {
    Ok((
        directory.join(text(&stage["file"], "browser capture")?),
        stage["state"]["geometry"].clone(),
        stage["state"]["records"].clone(),
        stage["ready"].clone(),
        stage.get("work").cloned().unwrap_or_else(|| json!({})),
    ))
}

fn validate_actions(run: &Value, expected: &[&str], engine: &str) -> Result<()> {
    let actual: Vec<_> = array(&run["stages"], "stages")?
        .iter()
        .filter_map(|stage| stage["action"].as_str())
        .collect();
    if actual != expected {
        return Err(format!("incomplete {engine} actions"));
    }
    Ok(())
}

fn issue(issues: &mut Vec<Value>, engine: &str, action: &str, message: &str) {
    issues.push(json!({"engine": engine, "action": action, "issue": message}));
}

fn round_if(value: Value, enabled: bool) -> Value {
    if enabled {
        value
            .as_f64()
            .and_then(|number| serde_json::Number::from_f64(number.round_ties_even()))
            .map(Value::Number)
            .unwrap_or(value)
    } else {
        value
    }
}

fn select(value: &Value, names: &[&str]) -> Value {
    Value::Object(
        names
            .iter()
            .map(|name| ((*name).into(), value[*name].clone()))
            .collect(),
    )
}

fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} must be an array"))
}

fn text<'a>(value: &'a Value, label: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} must be a string"))
}

fn integer(value: &Value, label: &str) -> Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| format!("{label} must be an integer"))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .components()
        .collect()
}
