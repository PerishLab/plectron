use plectron_observe::{handoff, report, trace};
use serde_json::{Value, json};
use std::fs;

#[test]
fn warmup() {
    let mut warm = record(1, None, (0, 10), 1);
    warm["name"] = json!("warmup");
    let live = record(3, None, (20, 30), 1);
    let source = document(vec![warm, record(2, Some(1), (1, 9), 1), live.clone()]);
    assert_eq!(
        handoff::measured(&source).unwrap()["records"],
        json!([live])
    );

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
fn nesting() {
    let source = record(1, None, (0, 10), 1);
    let execution = record(2, None, (20, 100), 1);
    let mut child = record(3, Some(1), (30, 90), 1);
    child["enclosing"] = json!(2);
    let (summary, _) = report::analyze(&document(vec![source, execution, child])).unwrap();
    assert_eq!(summary["groups"]["1"]["self"]["total"], 10);
    assert_eq!(summary["groups"]["2"]["self"]["total"], 20);
    assert_eq!(summary["groups"]["3"]["self"]["total"], 60);

    let mut received = record(2, Some(1), (100, 140), 2);
    received["kind"] = json!({"type": "receive", "captured": 10, "thread": 1});
    let (summary, timeline) = report::analyze(&document(vec![
        record(1, None, (0, 20), 1),
        received.clone(),
    ]))
    .unwrap();
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

    let (lost, _) = report::analyze(&document(vec![record(2, Some(1), (10, 20), 1)])).unwrap();
    assert_eq!(lost["complete"], false);
    assert!(lost["groups"]["2"]["self"].is_null());
    assert!(report::analyze(&document(vec![record(1, None, (0, 10), 1); 2])).is_err());

    let accounting = document(vec![
        record(1, None, (0, 100), 1),
        record(2, Some(1), (10, 40), 1),
        record(3, Some(1), (30, 50), 1),
        record(4, Some(1), (20, 90), 2),
    ]);
    let (summary, timeline) = report::analyze(&accounting).unwrap();
    assert_eq!(summary["groups"]["1"]["self"]["total"], 60);
    assert_eq!(timeline["traceEvents"].as_array().unwrap().len(), 4);

    let mut failed = record(2, Some(1), (5, 10), 1);
    failed["outcome"] = json!("failure");
    let (summary, _) =
        report::analyze(&document(vec![record(1, None, (0, 100), 1), failed])).unwrap();
    assert_eq!(summary["successful"], true);
    assert_eq!(summary["groups"]["2"]["outcomes"], json!({"failure": 1}));
    assert!(report::analyze(&document(Vec::new())).unwrap().0["successful"].is_null());
}

#[test]
fn narrowest() {
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
fn references() {
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

fn record(identity: i64, parent: Option<i64>, (start, end): (i64, i64), thread: i64) -> Value {
    json!({
        "id": identity, "parent": parent, "start": start, "end": end, "thread": thread,
        "cpu": null, "outcome": "success", "name": identity.to_string(), "kind": {"type": "scope"},
    })
}

fn document(records: Vec<Value>) -> Value {
    json!({"version": 1, "unit": "nanoseconds", "stopped": true, "open": 0, "dropped": 0, "process": 1, "records": records})
}

fn batch(round: u64, notified: bool, value: u64) -> Value {
    let samples = json!([{"wall": value, "waits": 1}]);
    let run = json!({"case": "probe", "traced": false, "notified": notified, "samples": samples, "report": null});
    json!({"round": round, "run": run})
}
