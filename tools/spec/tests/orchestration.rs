use serde_json::json;
use spec::orchestration::{dispatch, stream};

#[test]
fn dispatch_groups_policies_and_retains_attempt_work() {
    let samples = vec![
        json!({"policy": "hedge", "delay": 4, "content": 10, "preparation": 20, "ready": 30, "selected": 40, "winner": "metal", "duplicate": true, "metal": {"start": 1, "work": 5}, "cpu": {"start": 2, "work": 7}}),
        json!({"policy": "hedge", "delay": 4, "content": 20, "preparation": 30, "ready": 40, "selected": 50, "winner": "cpu", "duplicate": false, "metal": {"start": 3, "work": 9}, "cpu": {"start": 4, "work": 11}}),
    ];
    let summary = dispatch::summarize(&samples).unwrap();
    let group = &summary["hedge:4"];
    assert_eq!(group["samples"], 2);
    assert_eq!(group["timing"]["content"]["median"], 15.0);
    assert_eq!(group["timing"]["metal_work"]["maximum"], 9);
    assert_eq!(group["winners"], json!({"metal": 1, "cpu": 1}));
    assert_eq!(group["duplicates"], 1);
}

#[test]
fn stream_reports_process_round_and_artifact_distributions() {
    let samples = vec![
        json!({"backend": "executor", "lanes": 2, "rounds": 2, "total": 40, "samples": [10, 20]}),
        json!({"backend": "executor", "lanes": 2, "rounds": 2, "total": 80, "samples": [30, 40]}),
    ];
    let summary = stream::summarize(&samples).unwrap();
    let group = &summary["executor:2"];
    assert_eq!(group["processes"], 2);
    assert_eq!(group["artifact"]["median"], 15.0);
    assert_eq!(group["round"]["median"], 25.0);
}
