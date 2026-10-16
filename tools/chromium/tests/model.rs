use plectron_chromium::reference::{ACTIONS, identity, validate};
use plectron_chromium::summary::{Event, duration, spread, topology, union};
use serde_json::{Value, json};

#[test]
fn accounting() {
    let values = (1..=20).map(|value| json!(value)).collect::<Vec<_>>();
    assert_eq!(
        spread(&values).unwrap(),
        json!({
            "samples": 20, "minimum": 1, "median": 10.5, "p95": 19, "maximum": 20,
        })
    );
    assert_eq!(
        union(&[(0.0, 10.0), (2.0, 4.0), (8.0, 12.0), (20.0, 25.0)]),
        17.0
    );
    assert_eq!(union(&[]), 0.0);
    let events = [1, 1, 2].map(|tid| event(tid, "X", "task", (0.0, 1_000.0)));
    assert_eq!(duration(&events), 2.0);
}

#[test]
fn clipping() {
    let mut events = [("start", 1_000.0), ("script", 2_000.0), ("end", 5_000.0)]
        .map(|(name, time)| event(1, "I", &format!("case:{name}"), (time, 0.0)))
        .to_vec();
    events.extend([
        event(1, "X", "RunTask", (0.0, 3_000.0)),
        event(1, "X", "RunTask", (4_000.0, 2_000.0)),
        event(1, "X", "Layout", (2_000.0, 500.0)),
        event(1, "X", "Paint", (4_000.0, 500.0)),
        event(1, "X", "Paint", (4_100.0, 100.0)),
        event(2, "X", "Layout", (2_000.0, 1_000.0)),
        event(2, "X", "RasterTask", (2_500.0, 2_000.0)),
    ]);
    let result = topology(&events, "case").unwrap();
    assert_eq!(result["main"], 3.0);
    assert_eq!(result["named"], 2.0);
    assert_eq!(result["other"], 1.0);
    assert_eq!(result["phases"]["Paint"], 0.5);
    assert_eq!(result["raster"], 2.0);
    assert_eq!(result["window"], 4.0);
    assert!(topology(&events[1..], "case").is_err());
}

#[test]
fn identities() {
    let mut report = json!({
        "version": "identity", "browser": "identity", "host": "identity", "sources": "identity",
        "reset": "identity", "adaptations": "identity", "differences": "identity",
        "endpoint": "identity", "accounting": "identity", "revision": "identity",
        "config": {"count": 384, "warmups": 3, "runs": 20, "url": "http://localhost:1234/"},
        "gpu": {"auxAttributes": {"initializationTime": 0.1, "renderer": "Metal"}},
    });
    let mut changed = report.clone();
    changed["config"]["url"] = json!("http://localhost:5678/");
    changed["gpu"]["auxAttributes"]["initializationTime"] = json!(0.2);
    assert_eq!(identity(&report).unwrap(), identity(&changed).unwrap());
    changed["sources"] = json!("different harness");
    assert_ne!(identity(&report).unwrap(), identity(&changed).unwrap());
    report.as_object_mut().unwrap().remove("host");
    assert!(identity(&report).is_err());
}

#[test]
fn validation() {
    let original = reference();
    assert_eq!(
        validate(&original).unwrap().as_object().unwrap().len(),
        ACTIONS.len()
    );
    for mutation in ["missing", "duplicate", "state", "action", "error", "mode"] {
        let mut report = original.clone();
        match mutation {
            "missing" => {
                report["runs"].as_array_mut().unwrap().pop();
            }
            "duplicate" => {
                report["runs"].as_array_mut().unwrap().last_mut().unwrap()["index"] = json!(0)
            }
            "state" => {
                report["runs"].as_array_mut().unwrap().last_mut().unwrap()["stages"][0]["state"]["records"] =
                    json!(385)
            }
            "action" => report["runs"][0]["stages"]
                .as_array_mut()
                .unwrap()
                .reverse(),
            "error" => report["errors"] = json!(["script failed"]),
            "mode" => report["runs"]
                .as_array_mut()
                .unwrap()
                .push(json!({"mode": "warmup"})),
            _ => unreachable!(),
        }
        assert!(validate(&report).is_err(), "mutation {mutation}");
    }
}

fn event(tid: i64, ph: &str, name: &str, (ts, dur): (f64, f64)) -> Event {
    Event {
        pid: 1,
        tid,
        ph: ph.into(),
        name: name.into(),
        ts,
        dur,
    }
}

fn reference() -> Value {
    let stages = json!(ACTIONS.map(|action| json!({"action": action, "state": {"records": 384}})));
    let runs: Vec<_> = ["plain", "trace"]
        .into_iter()
        .flat_map(|mode| [0, 1].map(|index| (mode, index)))
        .map(|(mode, index)| json!({"mode": mode, "index": index, "stages": stages}))
        .collect();
    json!({"config": {"runs": 2}, "errors": [], "runs": runs})
}
