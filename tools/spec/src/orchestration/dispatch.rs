use super::{Arguments, digest, execute, output, platform, stats, write};
use crate::Result;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn run(values: &[String]) -> Result<Value> {
    let args = Arguments::new(values);
    args.reject(&["--binary", "--scenario", "--runs", "--delays", "--output"])?;
    let binary = PathBuf::from(args.one("--binary").unwrap_or("target/release/spec"))
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let scenario = args.one("--scenario").unwrap_or("chat");
    let runs = args
        .one("--runs")
        .unwrap_or("20")
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    let delays = if args.many("--delays").is_empty() {
        vec![0, 4, 8, 12, 16]
    } else {
        args.many("--delays")
            .into_iter()
            .map(|value| value.parse::<u64>().map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>>>()?
    };
    let root = output(&PathBuf::from(
        args.one("--output").ok_or("--output is required")?,
    ))?;
    if runs == 0 {
        return Err("runs must be positive".into());
    }
    let mut policies = vec![
        ("serial", None),
        ("cpu", None),
        ("overlap", None),
        ("executor", None),
    ];
    policies.extend(delays.iter().map(|delay| ("hedge", Some(*delay))));
    write(
        &root.join("manifest.json"),
        &json!({
            "version": 1, "binary": binary, "sha256": digest(&binary)?, "scenario": scenario, "runs": runs,
            "policies": policies.iter().map(|(name, delay)| json!({"name": name, "delay_ms": delay})).collect::<Vec<_>>(),
            "platform": platform(), "process": "fresh process per sample; OS file and driver caches uncontrolled",
            "endpoint": "first completed compute artifact; readback and presentation excluded",
        }),
    )?;
    let mut samples = Vec::new();
    for (policy, delay) in policies {
        for index in 0..runs {
            eprintln!("{policy} {delay:?} {}/{}", index + 1, runs);
            let mut command = vec!["dispatch".into(), scenario.into(), policy.into()];
            if let Some(delay) = delay {
                command.push(delay.to_string());
            }
            let sample = execute(&binary, &command)?;
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
                sample["policy"].as_str().unwrap_or_default(),
                sample["delay"]
            ))
            .or_default()
            .push(sample);
    }
    let mut summary = Map::new();
    for (key, rows) in groups {
        let mut timing = Map::new();
        for name in ["content", "preparation", "ready", "selected"] {
            timing.insert(
                name.into(),
                stats(&rows.iter().map(|row| row[name].clone()).collect::<Vec<_>>())?,
            );
        }
        for backend in ["metal", "cpu"] {
            let attempts = rows
                .iter()
                .filter_map(|row| (!row[backend].is_null()).then_some(&row[backend]))
                .collect::<Vec<_>>();
            if !attempts.is_empty() {
                timing.insert(
                    format!("{backend}_start"),
                    stats(
                        &attempts
                            .iter()
                            .map(|row| row["start"].clone())
                            .collect::<Vec<_>>(),
                    )?,
                );
                timing.insert(
                    format!("{backend}_work"),
                    stats(
                        &attempts
                            .iter()
                            .map(|row| row["work"].clone())
                            .collect::<Vec<_>>(),
                    )?,
                );
            }
        }
        summary.insert(key, json!({
            "samples": rows.len(), "timing": timing,
            "winners": {"metal": rows.iter().filter(|row| row["winner"] == "metal").count(), "cpu": rows.iter().filter(|row| row["winner"] == "cpu").count()},
            "duplicates": rows.iter().filter(|row| row["duplicate"].as_bool() == Some(true)).count(),
        }));
    }
    Ok(Value::Object(summary))
}
