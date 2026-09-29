use plectron_chromium::compare::{compare_with, projection, validate as validate_pair};
use plectron_chromium::paired::resources;
use plectron_chromium::reference::{ACTIONS, identity, validate};
use plectron_chromium::summary::{Event, duration, spread, topology, union};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[test]
fn distributions_and_intervals_preserve_accounting() {
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
    let events = [1, 1, 2].map(|tid| event(1, tid, "X", "task", 0.0, 1_000.0));
    assert_eq!(duration(&events), 2.0);
}

#[test]
fn topology_clips_threads_and_requires_markers() {
    let mut events = [("start", 1_000.0), ("script", 2_000.0), ("end", 5_000.0)]
        .map(|(name, time)| event(1, 1, "I", &format!("case:{name}"), time, 0.0))
        .to_vec();
    events.extend([
        event(1, 1, "X", "RunTask", 0.0, 3_000.0),
        event(1, 1, "X", "RunTask", 4_000.0, 2_000.0),
        event(1, 1, "X", "Layout", 2_000.0, 500.0),
        event(1, 1, "X", "Paint", 4_000.0, 500.0),
        event(1, 1, "X", "Paint", 4_100.0, 100.0),
        event(1, 2, "X", "Layout", 2_000.0, 1_000.0),
        event(1, 2, "X", "RasterTask", 2_500.0, 2_000.0),
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
fn reference_identity_ignores_ephemeral_endpoint_details() {
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
fn reference_validation_rejects_incomplete_or_changed_runs() {
    let original = reference_report();
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

#[test]
fn resources_include_stylesheet_fonts_and_requests() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for name in [
        "gallery/main.js",
        "gallery/style.css",
        "fonts/Regular.ttf",
        "pictures/dawn.png",
        "benchmarks/normalize.css",
    ] {
        write_file(root, name, b"x");
    }
    fs::write(
        root.join("gallery/style.css"),
        "src:url('../fonts/Regular.ttf')",
    )
    .unwrap();
    let result = resources(
        root,
        "gallery",
        "normalized",
        &[
            "gallery/index.html".into(),
            "pictures/dawn.png".into(),
            "missing.png".into(),
        ],
    )
    .unwrap();
    let names = result.as_object().unwrap();
    assert_eq!(names.len(), 5);
    assert!(names.contains_key("fonts/Regular.ttf"));
    assert!(names.contains_key("pictures/dawn.png"));
    assert_eq!(
        names["gallery/main.js"]["fingerprint"],
        json!(0xAF63BD4C8601B7A7_u64)
    );
    assert_eq!(names["gallery/style.css"]["bytes"], 42);
}

#[test]
fn flow_projection_retains_only_shared_semantics() {
    let value = json!({"boxes": {"region": [24, 196, 680, 384], "record-0": [44, 216, 640, 27]}, "scroll": 48, "extent": 4096});
    let mut changed = value.clone();
    changed["boxes"]["record-0"][1] = json!(224);
    changed["extent"] = json!(3584);
    assert_eq!(
        projection("article", &value),
        projection("article", &changed)
    );
    changed["boxes"]["record-0"][3] = json!(54);
    assert_ne!(
        projection("article", &value),
        projection("article", &changed)
    );

    let native = json!({"boxes": {"region": [20, 148, 680, 320], "record-0": [20, 168, 680, 195]}, "scroll": 0, "extent": 24960});
    let browser = json!({"boxes": {"region": [20, 148, 680, 320], "record-0": [20, 168, 680, 195.1875]}, "scroll": 0, "extent": 24984});
    assert_eq!(
        projection("gallery", &native),
        projection("gallery", &browser)
    );
}

#[test]
fn paired_validation_and_freshness_are_hard_checks() {
    let (native, browser) = paired_reports();
    validate_pair(&native, &browser).unwrap();
    for mutation in [
        "assets",
        "profile",
        "actions",
        "lifecycle",
        "empty",
        "viewport",
    ] {
        let mut changed = native.clone();
        match mutation {
            "assets" => {
                changed["runs"][0]["assets"]["app://scenarios/cards/main.js"]["fingerprint"] =
                    json!(3)
            }
            "profile" => changed["runs"][0]["config"]["scenario.normalized"] = json!(false),
            "actions" => {
                changed["runs"][0]["stages"].as_array_mut().unwrap().pop();
            }
            "lifecycle" => changed["runs"][0]["closed"] = json!(false),
            "empty" => {
                changed["repetitions"] = json!(0);
                changed["runs"] = json!([]);
            }
            "viewport" => changed["case"]["viewport"] = json!([640, 480]),
            _ => unreachable!(),
        }
        assert!(
            validate_pair(&changed, &browser).is_err(),
            "mutation {mutation}"
        );
    }

    for mutation in ["none", "stale", "oracle", "geometry"] {
        let (native, mut browser) = paired_reports();
        match mutation {
            "stale" => {
                browser["runs"][0]["stages"][2]["file"] = json!("seed");
                browser["runs"][0]["stages"][2]["oracle"] = json!("seed");
            }
            "oracle" => browser["runs"][0]["stages"][2]["oracle"] = json!("seed"),
            "geometry" => {
                browser["runs"][0]["stages"][2]["state"]["geometry"] = json!({"different": true})
            }
            _ => {}
        }
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("native.json"),
            serde_json::to_vec(&native).unwrap(),
        )
        .unwrap();
        fs::write(
            directory.path().join("report.json"),
            serde_json::to_vec(&browser).unwrap(),
        )
        .unwrap();
        let result = compare_with(
            &directory.path().join("native.json"),
            directory.path(),
            |path| Ok(path.file_name().unwrap().to_string_lossy().into()),
        )
        .unwrap();
        assert_eq!(
            result["passed"],
            json!(mutation == "none"),
            "mutation {mutation}"
        );
        assert!(result["ratio"].is_null());
    }
}

fn event(pid: i64, tid: i64, ph: &str, name: &str, ts: f64, dur: f64) -> Event {
    Event {
        pid,
        tid,
        ph: ph.into(),
        name: name.into(),
        ts,
        dur,
    }
}

fn reference_report() -> Value {
    let runs = ["plain", "trace"]
        .into_iter()
        .flat_map(|mode| {
            (0..2).map(move |index| json!({
        "mode": mode,
        "index": index,
        "stages": ACTIONS.map(|action| json!({"action": action, "state": {"records": 384}})),
    }))
        })
        .collect::<Vec<_>>();
    json!({"config": {"runs": 2}, "errors": [], "runs": runs})
}

fn paired_reports() -> (Value, Value) {
    let resources = json!({
        "cards/main.js": {"bytes": 1, "fingerprint": 2},
        "cards/style.css": {"bytes": 1, "fingerprint": 2},
        "fonts/Lato-Regular.ttf": {"bytes": 1, "fingerprint": 2},
        "benchmarks/normalize.css": {"bytes": 1, "fingerprint": 2},
    });
    let geometry = json!({"boxes": {"region": [20, 168, 680, 320]}, "scroll": 0, "extent": 640});
    let stages = ["seed", "steady", "edit"].map(|action| json!({
        "action": action, "records": 8, "geometry": geometry,
        "capture": {"file": if action == "steady" { "seed" } else { action }, "ready": 1_000_000},
    }));
    let native = json!({
        "debug": false, "case": {"id": "cards", "actions": ["edit"], "viewport": [720, 640]},
        "count": 8, "edits": 1, "repetitions": 1, "capture": "native", "runs": [{
            "failure": null, "closed": true,
            "config": {"scenario.capture": true, "scenario.normalized": true},
            "assets": resources.as_object().unwrap().iter().map(|(key, value)| (format!("app://scenarios/{key}"), value.clone())).collect::<serde_json::Map<_, _>>(),
            "stages": stages,
        }],
    });
    let stages = ["seed", "steady", "edit"].map(|action| {
        json!({
            "action": action,
            "file": if action == "steady" { "seed" } else { action },
            "oracle": if action == "steady" { "seed" } else { action },
            "ready": 1,
            "state": {"records": 8, "geometry": geometry},
            "work": {},
        })
    });
    let browser = json!({
        "config": {"scenario": "cards", "count": 8, "runs": 1}, "errors": [],
        "resources": resources, "endpoint": "browser", "runs": [{"index": 0, "stages": stages}],
    });
    (native, browser)
}

fn write_file(root: &Path, name: &str, data: &[u8]) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}
