use super::EXTENT;
use crate::{Result, digest};
use serde_json::{Value, json};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

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
    let trials = integer(&browser["config"]["runs"], "browser runs")?;
    if repetitions == 0 || trials == 0 {
        return Err("empty repetitions".into());
    }
    let native = array(&report["runs"], "native runs")?;
    let chromium = array(&browser["runs"], "browser runs")?;
    if native.len() != repetitions as usize || chromium.len() != trials as usize {
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
        actions(run, &expected, "native")?;
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
    if indices != (0..trials).collect::<Vec<_>>() {
        return Err("duplicate or missing browser repetition".into());
    }
    for run in chromium {
        actions(run, &expected, "browser")?;
    }
    Ok(())
}

fn actions(run: &Value, expected: &[&str], engine: &str) -> Result<()> {
    let actual: Vec<_> = array(&run["stages"], "stages")?
        .iter()
        .filter_map(|stage| stage["action"].as_str())
        .collect();
    if actual != expected {
        return Err(format!("incomplete {engine} actions"));
    }
    Ok(())
}

pub fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} must be an array"))
}

pub fn text<'a>(value: &'a Value, label: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} must be a string"))
}

fn integer(value: &Value, label: &str) -> Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| format!("{label} must be an integer"))
}

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
