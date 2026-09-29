use plectron_observe::{activity, bench, feel, frames, handoff, ipc, passive, report, trace};
use serde_json::{Value, json};
use std::fs;

#[test]
fn frame_receipt_and_equivalence_are_explicit() {
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
fn ipc_distribution_and_aggregation_preserve_endpoints() {
    assert_eq!(
        ipc::distribution(&(1..=20).map(|value| json!(value)).collect::<Vec<_>>()).unwrap(),
        json!({"median": 10.5, "p95": 19.0, "maximum": 20.0}),
    );
    let result = |wall, admission| {
        json!({"actions": {"probe": {"timing": {
            "wall": {"median": wall}, "admission": {"median": admission},
        }}}})
    };
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
fn cpu_clock_parsing_matches_process_accounting() {
    assert_eq!(activity::cputime("1:02.50").unwrap(), 62_500_000_000);
    assert_eq!(
        activity::cputime("2-03:04:05.25").unwrap(),
        183_845_250_000_000
    );
}

#[test]
fn feel_summary_and_assessment_keep_missing_evidence_visible() {
    let frame = |identity, admitted, produced, visible: Option<i64>, drops| {
        json!({
            "id": identity,
            "target": {"type": "frame", "unit": 1, "frame": identity, "scene": 1},
            "state": {"type": "complete", "result": {"Ok": ""}},
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
            "forward",
            1.0
        )["valid"],
        true
    );
    let interrupted = json!({"frames": 10, "evicted": [], "missing_visibility": 1, "drops": {"total": 1}, "visible_span": 100_000_000});
    let result = feel::assess(
        &interrupted,
        &json!({"settled": false}),
        &json!({"complete": true, "successful": true}),
        "forward",
        1.0,
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
fn handoff_excludes_warmup_and_requires_paired_batches() {
    let mut warm = record(1, None, 0, 10, 1);
    warm["name"] = json!("warmup");
    let live = record(3, None, 20, 30, 1);
    let source = document(vec![warm, record(2, Some(1), 1, 9, 1), live.clone()]);
    assert_eq!(
        handoff::measured(&source).unwrap()["records"],
        json!([live])
    );

    let batch = |round, notified, value| {
        json!({"round": round, "run": {
            "case": "probe", "traced": false, "notified": notified,
            "samples": [{"wall": value, "waits": 1}], "report": null,
        }})
    };
    let mut source = json!({
        "unit": "nanoseconds", "runs": [batch(0, false, 10), batch(0, true, 1), batch(1, false, 20), batch(1, true, 4)],
        "endpoint": "probe", "build": "test", "samples": 1, "rounds": 2, "limits": [], "idle": [],
    });
    let result = handoff::analyze(&source).unwrap();
    let wall = result["comparison"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["metric"] == "wall")
        .unwrap();
    assert_eq!(wall["paired_delta"], -12.5);
    assert_eq!(wall["paired_percent"], -85.0);
    source["runs"].as_array_mut().unwrap().pop();
    assert!(handoff::analyze(&source).is_err());
}

#[test]
fn benchmark_aggregation_rejects_unpaired_or_absent_metrics() {
    let entry = |value: Option<i64>| json!([{"id": "example", "summary": {"unit": "milliseconds", "actions": {"edit": {"timing": {"total": value.map(|value| json!({"median": value})).unwrap_or(Value::Null)}}}}}]);
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
fn report_tracks_execution_nesting_handoffs_and_loss() {
    let source = record(1, None, 0, 10, 1);
    let execution = record(2, None, 20, 100, 1);
    let mut child = record(3, Some(1), 30, 90, 1);
    child["enclosing"] = json!(2);
    let (summary, _) = report::analyze(&document(vec![source, execution, child])).unwrap();
    assert_eq!(summary["groups"]["1"]["self"]["total"], 10);
    assert_eq!(summary["groups"]["2"]["self"]["total"], 20);
    assert_eq!(summary["groups"]["3"]["self"]["total"], 60);

    let mut received = record(2, Some(1), 100, 140, 2);
    received["kind"] = json!({"type": "receive", "captured": 10, "thread": 1});
    let (summary, timeline) =
        report::analyze(&document(vec![record(1, None, 0, 20, 1), received.clone()])).unwrap();
    assert_eq!(summary["groups"]["2"]["handoff"]["total"], 90);
    assert_eq!(summary["groups"]["2"]["wall"]["total"], 40);
    let flow = timeline["traceEvents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| matches!(event["ph"].as_str(), Some("s" | "f")))
        .collect::<Vec<_>>();
    assert_eq!(flow[0]["tid"], 1);
    assert_eq!(flow[1]["tid"], 2);
    received["kind"]["captured"] = json!(101);
    assert!(report::analyze(&document(vec![received])).is_err());
    assert_eq!(report::union(&[(0, 5), (3, 8), (12, 14)]), 10);

    let (lost, _) = report::analyze(&document(vec![record(2, Some(1), 10, 20, 1)])).unwrap();
    assert_eq!(lost["complete"], false);
    assert!(lost["groups"]["2"]["self"].is_null());
    assert!(report::analyze(&document(vec![record(1, None, 0, 10, 1); 2])).is_err());

    let accounting = document(vec![
        record(1, None, 0, 100, 1),
        record(2, Some(1), 10, 40, 1),
        record(3, Some(1), 30, 50, 1),
        record(4, Some(1), 20, 90, 2),
    ]);
    let (summary, timeline) = report::analyze(&accounting).unwrap();
    assert_eq!(summary["groups"]["1"]["self"]["total"], 60);
    assert_eq!(timeline["traceEvents"].as_array().unwrap().len(), 4);

    let mut failed = record(2, Some(1), 5, 10, 1);
    failed["outcome"] = json!("failure");
    let (summary, _) =
        report::analyze(&document(vec![record(1, None, 0, 100, 1), failed])).unwrap();
    assert_eq!(summary["successful"], true);
    assert_eq!(summary["groups"]["2"]["outcomes"], json!({"failure": 1}));
    assert!(report::analyze(&document(Vec::new())).unwrap().0["successful"].is_null());
}

#[test]
fn native_samples_choose_the_narrowest_active_span() {
    let windows = [
        json!({"id": 1, "start": 0, "end": 100, "thread": 7}),
        json!({"id": 2, "start": 10, "end": 30, "thread": 7}),
        json!({"id": 3, "start": 15, "end": 25, "thread": 8}),
    ];
    let samples = [5, 20, 30, 100]
        .map(|time| json!({"time": time, "thread": 7}))
        .into_iter()
        .chain([json!({"time": 20, "thread": 8})])
        .collect::<Vec<_>>();
    let assigned = trace::assign(&windows, &samples).unwrap();
    assert_eq!(
        assigned
            .iter()
            .map(|sample| sample["span"].clone())
            .collect::<Vec<_>>(),
        vec![json!(1), json!(2), json!(1), Value::Null, json!(3)]
    );
}

#[test]
fn native_xml_resolves_references_and_reports_missing_spans() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.xml");
    let keys = [
        "subsystem",
        "process",
        "identifier",
        "thread",
        "time",
        "event-type",
        "message",
    ];
    let schema = keys
        .map(|key| format!("<col><mnemonic>{key}</mnemonic></col>"))
        .join("");
    let begin = "<subsystem id=\"1\">io.plectron.observe</subsystem><process id=\"2\"><pid>10</pid></process><identifier id=\"3\">42</identifier><thread id=\"4\"><tid>11</tid></thread><time>100</time><event>Begin</event><metadata><uint64>9</uint64><uint64>7</uint64></metadata>";
    let end = "<subsystem ref=\"1\"/><process ref=\"2\"/><identifier ref=\"3\"/><thread ref=\"4\"/><time>200</time><event>End</event><metadata/>";
    fs::write(&path, format!("<trace-query-result><node><schema>{schema}</schema><row>{begin}</row><row>{end}</row></node></trace-query-result>")).unwrap();
    let mut source = json!({"session": 9, "process": 10, "records": [{"id": 42, "parent": 7, "native": 11, "signpost": true}]});
    let (windows, missing) = trace::intervals(&path, &source).unwrap();
    assert_eq!(missing, Vec::<i64>::new());
    assert_eq!(
        windows,
        vec![json!({"id": 42, "start": 100, "end": 200, "thread": 11})]
    );
    source["session"] = json!(8);
    assert!(trace::intervals(&path, &source).is_err());

    fs::write(&path, "<trace-query-result><node><schema><col><mnemonic>subsystem</mnemonic></col></schema></node></trace-query-result>").unwrap();
    source["session"] = json!(9);
    assert_eq!(
        trace::intervals(&path, &source).unwrap(),
        (Vec::new(), vec![42])
    );
}

#[test]
fn passive_timestamps_require_complete_monotonic_topology() {
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

fn record(identity: i64, parent: Option<i64>, start: i64, end: i64, thread: i64) -> Value {
    json!({
        "id": identity, "parent": parent, "start": start, "end": end, "thread": thread,
        "cpu": null, "outcome": "success", "name": identity.to_string(), "kind": {"type": "scope"},
    })
}

fn document(records: Vec<Value>) -> Value {
    json!({"version": 1, "unit": "nanoseconds", "stopped": true, "open": 0, "dropped": 0, "process": 1, "records": records})
}
