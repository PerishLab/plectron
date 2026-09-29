use super::{Arguments, digest, execute, output, platform, stats, write};
use crate::Result;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn run(values: &[String]) -> Result<Value> {
    let args = Arguments::new(values);
    args.reject(&[
        "--binary",
        "--scenario",
        "--runs",
        "--rounds",
        "--lanes",
        "--output",
    ])?;
    let binary = PathBuf::from(args.one("--binary").unwrap_or("target/release/spec"))
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let scenario = args.one("--scenario").unwrap_or("chat");
    let runs = parse(args.one("--runs").unwrap_or("20"), "runs")?;
    let rounds = parse(args.one("--rounds").unwrap_or("20"), "rounds")?;
    let lanes = if args.many("--lanes").is_empty() {
        vec![1, 2, 4]
    } else {
        args.many("--lanes")
            .into_iter()
            .map(|value| parse(value, "lanes"))
            .collect::<Result<Vec<_>>>()?
    };
    if runs == 0
        || !(1..=1_000).contains(&rounds)
        || lanes.iter().any(|lanes| !(1..=8).contains(lanes))
    {
        return Err("runs must be positive, rounds 1..=1000 and lanes 1..=8".into());
    }
    let root = output(&PathBuf::from(
        args.one("--output").ok_or("--output is required")?,
    ))?;
    write(
        &root.join("manifest.json"),
        &json!({
            "version": 1, "binary": binary, "sha256": digest(&binary)?, "scenario": scenario, "runs": runs,
            "rounds": rounds, "lanes": lanes, "backends": ["metal", "executor"], "platform": platform(),
            "process": "fresh process per sample; one unmeasured warmup", "endpoint": "compute completion; preparation, readback and presentation excluded",
        }),
    )?;
    let mut samples = Vec::new();
    for backend in ["metal", "executor"] {
        for lanes in &lanes {
            for index in 0..runs {
                eprintln!("{backend} lanes={lanes} {}/{}", index + 1, runs);
                let sample = execute(
                    &binary,
                    &[
                        "stream".into(),
                        scenario.into(),
                        backend.into(),
                        rounds.to_string(),
                        lanes.to_string(),
                    ],
                )?;
                if sample["maximum"]
                    .as_f64()
                    .is_some_and(|maximum| maximum > 1.0)
                {
                    return Err("CPU and Metal artifacts diverged".into());
                }
                samples.push(sample);
                write(&root.join("samples.json"), &Value::Array(samples.clone()))?;
            }
        }
    }
    let result = json!({"unit": "nanoseconds", "summary": summarize(&samples)?});
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}

pub fn summarize(samples: &[Value]) -> Result<Value> {
    let mut groups: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for sample in samples {
        groups
            .entry(format!(
                "{}:{}",
                sample["backend"].as_str().unwrap_or_default(),
                sample["lanes"]
            ))
            .or_default()
            .push(sample);
    }
    let mut result = Map::new();
    for (key, rows) in groups {
        let rounds = rows[0]["rounds"].as_u64().ok_or("rounds are absent")?;
        let lanes = rows[0]["lanes"].as_u64().ok_or("lanes are absent")?;
        let artifacts = rows
            .iter()
            .map(|row| json!(row["total"].as_f64().unwrap_or(f64::NAN) / (rounds * lanes) as f64))
            .collect::<Vec<_>>();
        let roundsamples = rows
            .iter()
            .flat_map(|row| row["samples"].as_array().into_iter().flatten().cloned())
            .collect::<Vec<_>>();
        result.insert(key, json!({"processes": rows.len(), "rounds": rounds, "lanes": lanes, "total": stats(&rows.iter().map(|row| row["total"].clone()).collect::<Vec<_>>())?, "artifact": stats(&artifacts)?, "round": stats(&roundsamples)?}));
    }
    Ok(Value::Object(result))
}

fn parse(value: &str, label: &str) -> Result<usize> {
    value.parse().map_err(|error| format!("{label}: {error}"))
}
