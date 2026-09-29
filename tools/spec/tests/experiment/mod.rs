mod plans;
mod totals;
use serde_json::{Value, json};
use spec::experiment::{Document, Group, Rules, compare};

fn stage(action: &str) -> Value {
    let mut stage = json!({
        "action": action, "inspection": 250000, "evaluation": 1000000, "publication": 3000000, "total": 10000000,
        "update": {"elapsed": 500000}, "reuse": {"measurement": 500000, "assembly": 500000,
            "shaping": {"preparations": 1, "shapes": 1, "bytes": 5, "candidates": 0, "materialized": 1}},
        "painting": {"elapsed": 1000000, "requests": 8, "hits": 7, "recorded": 1, "glyphs": 5, "groups": 4},
        "rendering": {"preparation": 2000000, "submission": 1000000, "completion": 4000000,
            "gpu": 500000, "checksum": 123, "maximum": 1}
    });
    if action == "steady" {
        stage["rendering"] = Value::Null;
        stage["reuse"] = Value::Null;
        stage["painting"] = Value::Null;
        stage["total"] = json!(4000000);
    }
    stage
}
fn fixture() -> Value {
    let stages: Vec<_> = ["seed", "steady", "append", "edit", "reorder", "remove"]
        .into_iter()
        .map(stage)
        .collect();
    let run = json!({"closed": true, "failure": null, "config": {}, "load": 12000000,
        "startup": {"resources": 1000000, "fonts": 6000000, "pictures": 6000000,
            "loaded": 6000000, "runtime": 7000000, "activation": 8000000,
            "scene": 9000000, "publication": 10000000},
        "loading": {"requests": 4000000, "parsing": 900000, "registration": 100000,
            "fonts": 2, "bytes": 12000000},
        "assets": {"app://scenarios/a": {"bytes": 12, "fingerprint": 123, "requests": 1}}, "stages": stages});
    json!({"repetitions": 2, "case": {"id": "example", "entry": "example/index.html", "prefix": "item-",
        "region": "list", "actions": ["append", "edit", "reorder", "remove"], "viewport": [720,640]},
        "count": 6, "edits": 1, "backend": "metal", "architecture": "test", "system": "test",
        "debug": false, "unit": "nanoseconds", "rasterization": "local-quadratic-4x4",
        "endpoint": "completion", "accounting": "evaluation+publication+preparation+completion",
        "runs": [run.clone(), run], "summary": {"edit": {"median": 999}}})
}
fn comparison(a: Value, b: Value, rules: Rules) -> spec::experiment::Comparison {
    compare(
        &Document::parse(a).unwrap(),
        &Document::parse(b).unwrap(),
        &rules,
    )
}
#[test]
fn summaries() {
    let mut value = fixture();
    value["runs"][1]["stages"][3]["publication"] = json!(5000000);
    value["runs"][1]["stages"][3]["total"] = json!(12000000);
    let summary = Document::parse(value).unwrap().summary();
    assert!(summary.valid());
    assert_eq!(summary.runs, 2);
    assert_eq!(summary.startup["fonts"].as_ref().unwrap().median, 5.0);
    assert_eq!(summary.startup["fontrequest"].as_ref().unwrap().median, 4.0);
    assert_eq!(summary.startup["fontparse"].as_ref().unwrap().median, 0.9);
    let edit = &summary.actions["edit"];
    assert_eq!(edit.timing["total"].as_ref().unwrap().median, 11.0);
    assert_eq!(edit.timing["publication"].as_ref().unwrap().median, 4.0);
    assert_eq!(edit.timing["paint"].as_ref().unwrap().median, 1.0);
    assert_eq!(edit.timing["inspection"].as_ref().unwrap().median, 0.25);
    assert_eq!(edit.timing["total"].as_ref().unwrap().p95, 12.0);
    assert_eq!(summary.actions["steady"].frames, 0);
    assert!(summary.actions["steady"].timing["preparation"].is_none());
    assert!(summary.actions["steady"].timing["paint"].is_none());
    assert_eq!(edit.maximum, Some(1));
}
#[test]
fn equality() {
    let result = comparison(fixture(), fixture(), Rules::default());
    assert!(result.compatible && result.passed);
    assert_eq!(result.actions["edit"].output.equal, Some(true));
    let mut changed = fixture();
    changed["runs"][1]["stages"][3]["rendering"]["checksum"] = json!(124);
    let result = comparison(fixture(), changed, Rules::default());
    assert!(result.compatible && !result.passed);
    assert_eq!(result.actions["edit"].output.after.len(), 2);
}
#[test]
fn work() {
    let mut changed = fixture();
    for run in changed["runs"].as_array_mut().unwrap() {
        run["stages"][3]["reuse"]["shaping"]["shapes"] = json!(2);
    }
    let result = comparison(fixture(), changed.clone(), Rules::default());
    assert!(result.passed);
    assert_eq!(
        result.actions["edit"].work["/reuse/shaping/shapes"].equal,
        Some(false)
    );
    let rules = Rules {
        same: vec![Group::Shaping],
        ..Rules::default()
    };
    let result = comparison(fixture(), changed, rules);
    assert!(result.compatible && !result.passed);
    let rules = Rules {
        same: vec![Group::Geometry],
        ..Rules::default()
    };
    assert!(!comparison(fixture(), fixture(), rules).compatible);
}
#[test]
fn timing() {
    let mut changed = fixture();
    for run in changed["runs"].as_array_mut().unwrap() {
        run["stages"][3]["publication"] = json!(13000000);
        run["stages"][3]["total"] = json!(20000000);
    }
    let result = comparison(fixture(), changed, Rules::default());
    assert!(result.passed);
    let delta = &result.actions["edit"].timing["total"];
    assert_eq!(delta.absolute, Some(10.0));
    assert_eq!(delta.percent, Some(100.0));
}
#[test]
fn observations() {
    let rules = Rules {
        same: vec![Group::Shaping],
        ..Rules::default()
    };
    let mut value = fixture();
    for run in value["runs"].as_array_mut().unwrap() {
        run["stages"][3]["reuse"]["shaping"]["shapes"] = json!(0);
    }
    assert!(comparison(value.clone(), value.clone(), rules.clone()).passed);
    for run in value["runs"].as_array_mut().unwrap() {
        run["stages"][3]["reuse"]["shaping"]["shapes"] = Value::Null;
    }
    assert!(!comparison(value.clone(), value, rules).compatible);
}
#[test]
fn incomplete() {
    let mutations = [
        "/runs/0/closed",
        "/runs/0/stages/3/rendering/checksum",
        "/backend",
        "/debug",
        "/runs/0/assets/app:~1~1scenarios~1a/fingerprint",
        "/runs/0/stages/3/total",
    ];
    for path in mutations {
        let mut changed = fixture();
        *changed.pointer_mut(path).unwrap() = Value::Null;
        assert!(
            !comparison(fixture(), changed, Rules::default()).passed,
            "{path}"
        );
    }
    let mut changed = fixture();
    changed["runs"][0]
        .as_object_mut()
        .unwrap()
        .remove("failure");
    assert!(!comparison(fixture(), changed, Rules::default()).passed);
    let mut changed = fixture();
    changed["runs"].as_array_mut().unwrap().pop();
    assert!(!comparison(fixture(), changed, Rules::default()).compatible);
    let mut changed = fixture();
    changed["runs"][0]["stages"]
        .as_array_mut()
        .unwrap()
        .remove(3);
    assert!(!comparison(fixture(), changed, Rules::default()).compatible);
    let mut changed = fixture();
    changed["runs"][0]["stages"]
        .as_array_mut()
        .unwrap()
        .swap(2, 3);
    assert!(!comparison(fixture(), changed, Rules::default()).compatible);
}
#[test]
fn metadata() {
    for (path, value) in [
        ("/case/viewport/0", json!(640)),
        ("/edits", json!(2)),
        ("/accounting", json!("other")),
        (
            "/runs/1/assets/app:~1~1scenarios~1a/fingerprint",
            json!(456),
        ),
    ] {
        let mut changed = fixture();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(!comparison(fixture(), changed, Rules::default()).compatible);
    }
    let mut changed = fixture();
    for run in changed["runs"].as_array_mut().unwrap() {
        run["config"]["paint.cache"] = json!({"read":false});
    }
    let result = comparison(fixture(), changed.clone(), Rules::default());
    assert!(!result.compatible);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.contains("after run 1: config.paint.cache changed: missing ->"))
    );
    let rules = Rules {
        vary: vec!["paint.cache".into()],
        ..Rules::default()
    };
    assert!(comparison(fixture(), changed, rules).passed);
}
#[test]
fn historical() {
    let mut old = fixture();
    old.as_object_mut().unwrap().remove("repetitions");
    assert!(!Document::parse(old.clone()).unwrap().summary().valid());
    for run in old["runs"].as_array_mut().unwrap() {
        run["config"]["scenario.runs"] = json!(2);
    }
    assert!(comparison(fixture(), old, Rules::default()).passed);
    let mut short = fixture();
    short["repetitions"] = json!(1);
    short["runs"].as_array_mut().unwrap().pop();
    assert!(comparison(fixture(), short, Rules::default()).passed);
}

#[test]
fn routing() {
    let mut value = fixture();
    for run in value["runs"].as_array_mut().unwrap() {
        run["stages"][3]["painting"]["routed"] = json!(1);
        run["stages"][3]["rendering"]["work"] = json!({"undemanded": 42});
        run["stages"][3]["reuse"]["construction"] = json!({"text":{"retained":12}});
    }
    let result = comparison(fixture(), value, Rules::default());
    let work = &result.actions["edit"].work;
    assert_eq!(
        serde_json::to_value(&work["/painting/routed"].before).unwrap(),
        json!(["missing"])
    );
    assert_eq!(
        serde_json::to_value(&work["/painting/routed"].after).unwrap(),
        json!([{"value":1}])
    );
    assert_eq!(
        serde_json::to_value(&work["/rendering/work/undemanded"].after).unwrap(),
        json!([{"value":42}])
    );
    assert_eq!(
        serde_json::to_value(&work["/reuse/construction/text/retained"].after).unwrap(),
        json!([{"value":12}])
    );
    assert!(result.passed);
}
