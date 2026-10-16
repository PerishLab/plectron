use plectron_observe::{activity, bench, feel, frames, ipc, passive};
use serde_json::{Value, json};

#[test]
fn receipt() {
    let snapshot = json!({
        "state": "ready", "scene": 2, "displayed": 2, "revision": 4,
        "presented": 4, "issued": null, "pending": null,
    });
    assert!(frames::settled(&snapshot));
    for (key, value) in [
        ("scene", Value::Null),
        ("displayed", json!(1)),
        ("presented", json!(3)),
        ("issued", json!(9)),
        ("pending", json!(10)),
    ] {
        let mut changed = snapshot.clone();
        changed[key] = value;
        assert!(!frames::settled(&changed), "{key}");
    }
    let counts = json!({"layouts": 0, "paints": 0, "checks": 0, "changes": 0, "queries": 0});
    let left = json!({"samples": [{"signature": "same", "computation": counts}]});
    frames::verify(&left, &left).unwrap();
    let mut changed = left.clone();
    changed["samples"][0]["signature"] = json!("different");
    assert!(frames::verify(&left, &changed).is_err());
    changed = left.clone();
    changed["samples"][0]["computation"]["layouts"] = json!(1);
    assert!(frames::verify(&left, &changed).is_err());
}

#[test]
fn endpoints() {
    assert_eq!(
        ipc::distribution(&(1..=20).map(|value| json!(value)).collect::<Vec<_>>()).unwrap(),
        json!({"median": 10.5, "p95": 19.0, "maximum": 20.0}),
    );
    let modes = json!({
        "baseline": result(20_000_000, 10_000_000),
        "internal": result(12_000_000, 10_000_000),
        "candidate": result(1_000_000, 400_000),
    });
    let rows = ipc::aggregate(&[json!({"false": modes, "true": modes})]).unwrap();
    let rows = rows.as_array().unwrap();
    let wall = rows
        .iter()
        .find(|row| row["mode"] == "candidate" && row["metric"] == "wall" && row["traced"] == false)
        .unwrap();
    assert_eq!(wall["median"], 1.0);
    assert_eq!(wall["paired_percent"], -95.0);
    let admission = rows
        .iter()
        .find(|row| {
            row["mode"] == "candidate" && row["metric"] == "admission" && row["traced"] == true
        })
        .unwrap();
    assert_eq!(admission["median"], 0.4);
}

#[test]
fn clock() {
    assert_eq!(activity::cputime("1:02.50").unwrap(), 62_500_000_000);
    assert_eq!(
        activity::cputime("2-03:04:05.25").unwrap(),
        183_845_250_000_000
    );
}

#[test]
fn evidence() {
    let complete = json!({"type": "complete", "result": {"Ok": ""}});
    let frame = |identity, admitted, produced, visible: Option<i64>, drops| {
        json!({
            "id": identity,
            "target": {"type": "frame", "unit": 1, "frame": identity, "scene": 1},
            "state": complete,
            "admitted": admitted, "produced": produced, "visible": visible,
            "detected": produced + 2, "drops": drops,
        })
    };
    let summary = feel::summarize(
        &[
            frame(1, 0, 10, Some(20), 0),
            frame(2, 30, 40, Some(55), 1),
            frame(3, 200_000_000, 200_000_010, None, 0),
        ],
        &[json!(9)],
    )
    .unwrap();
    assert_eq!(summary["frames"], 3);
    assert_eq!(summary["visible"], 2);
    assert_eq!(summary["missing_visibility"], 1);
    assert_eq!(summary["drops"], json!({"total": 1, "maximum": 1}));
    assert_eq!(summary["timing"]["admission_to_visibility"]["median"], 22.5);
    assert_eq!(
        summary["timing"]["active_visibility_cadence"]["median"],
        35.0
    );
    assert_eq!(summary["visible_span"], 35);
    let healthy = json!({"frames": 10, "evicted": [], "missing_visibility": 0, "drops": {"total": 0}, "visible_span": 900_000_000});
    assert_eq!(
        feel::assess(
            &healthy,
            &json!({"settled": true}),
            &json!({"complete": true, "successful": true}),
            &feel::Drive {
                mode: "forward",
                seconds: 1.0
            },
        )["valid"],
        true
    );
    let interrupted = json!({"frames": 10, "evicted": [], "missing_visibility": 1, "drops": {"total": 1}, "visible_span": 100_000_000});
    let result = feel::assess(
        &interrupted,
        &json!({"settled": false}),
        &json!({"complete": true, "successful": true}),
        &feel::Drive {
            mode: "forward",
            seconds: 1.0,
        },
    );
    assert_eq!(result["valid"], false);
    assert_eq!(result["reasons"].as_array().unwrap().len(), 3);
    assert_eq!(
        result["findings"],
        json!(["presentation reported dropped frames"])
    );
    assert!(feel::distribution(&[]).unwrap().is_null());
}

#[test]
fn pairing() {
    assert_eq!(
        bench::aggregate(&[json!({"off": entry(None), "wall": entry(None)})]).unwrap(),
        json!([])
    );
    let values = bench::aggregate(&[
        json!({"off": entry(Some(1)), "wall": entry(Some(3))}),
        json!({"off": entry(Some(10)), "wall": entry(Some(11))}),
    ])
    .unwrap();
    let wall = values
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["mode"] == "wall")
        .unwrap();
    assert_eq!(wall["paired_delta"], 1.5);
    assert_eq!(wall["paired_percent"], 105.0);
    assert_eq!(wall["medians"], json!([3.0, 11.0]));
    assert!(
        bench::aggregate(&[
            json!({"off": entry(Some(1)), "wall": entry(Some(1))}),
            json!({"off": entry(Some(1))})
        ])
        .is_err()
    );
}

#[test]
fn monotonic() {
    let task = json!({"admitted": 10, "produced": 20, "detected": 30});
    let frame = json!({"admitted": 25, "produced": 100, "detected": 110});
    let result = passive::durations(&task, &frame).unwrap();
    assert_eq!(result["owner_completion"], 100);
    assert_eq!(result["task_publication"], 10);
    assert_eq!(result["frame_production"], 75);
    let mut invalid = frame.clone();
    invalid["produced"] = Value::Null;
    assert!(passive::durations(&task, &invalid).is_err());
    invalid = frame;
    invalid["detected"] = json!(99);
    assert!(passive::durations(&task, &invalid).is_err());
}

fn result(wall: i64, admission: i64) -> Value {
    let timing = json!({"wall": {"median": wall}, "admission": {"median": admission}});
    let probe = json!({"probe": {"timing": timing}});
    json!({"actions": probe})
}

fn entry(value: Option<i64>) -> Value {
    let total = value
        .map(|value| json!({"median": value}))
        .unwrap_or(Value::Null);
    let edit = json!({"edit": {"timing": {"total": total}}});
    json!([{"id": "example", "summary": {"unit": "milliseconds", "actions": edit}}])
}
