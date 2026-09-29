#![cfg(target_os = "macos")]

mod process;
use process::Process;
use serde_json::{Value, json};

fn observe(process: &Process, command: Value) -> Value {
    process.control(&["observe", &command.to_string()])["status"].clone()
}
fn export(process: &Process, id: u64, name: &str) -> Value {
    let path = process.root.join(name);
    let output = process.command(&[
        "observe",
        &process.child.id().to_string(),
        &id.to_string(),
        path.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn reject(process: &Process, command: Value) {
    let request = json!({"version":1, "command":{"type":"observe", "command":command}});
    let response = process.raw(format!("{request}\n").as_bytes());
    assert!(response["result"]["Err"].is_string());
}
#[test]
fn lifecycle() {
    let mut process = Process::start();
    assert!(observe(&process, json!({"type":"status"})).is_null());
    let options = json!({"type":"start", "options":{"cpu":true}});
    let status = observe(&process, options.clone());
    let id = status["session"].as_u64().unwrap();
    reject(&process, options.clone());
    reject(&process, json!({"type":"discard", "id":id}));
    assert_eq!(
        process.evaluate("count = count + 1; console.log(count); count;")["Ok"],
        "11"
    );
    let held = process.control(&["hold"]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let snapshot = process.control(&["control"]);
        if snapshot["snapshot"]["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|job| job["id"] == held["job"] && job["phase"] == "held")
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
    }
    let stopped = observe(&process, json!({"type":"stop", "id":id}));
    assert_eq!(stopped["stopped"], true);
    assert!(stopped["open"].as_u64().unwrap() > 0);
    reject(&process, json!({"type":"discard", "id":id}));
    let partial = export(&process, id, "partial.json");
    assert!(partial["open"].as_u64().unwrap() > 0);
    process.control(&["unhold"]);
    process.completed(&held["job"]);
    let report = export(&process, id, "complete.json");
    assert_eq!(report["open"], 0);
    let records = report["records"].as_array().unwrap();
    let evaluation = records.iter().find(|r| r["name"] == "ipc.eval").unwrap();
    let execution = records
        .iter()
        .find(|r| r["name"] == "control.execute" && r["parent"] == evaluation["id"])
        .unwrap();
    assert_ne!(evaluation["thread"], execution["thread"]);
    assert!(
        records
            .iter()
            .any(|r| r["name"] == "js.execute" && r["parent"] == execution["id"])
    );
    assert!(records.iter().any(|r| r["name"] == "platform.request"));
    let publication = records
        .iter()
        .find(|r| r["name"] == "control.publish" && r["parent"] == execution["id"])
        .unwrap();
    assert!(
        records
            .iter()
            .any(|r| r["name"] == "control.accept" && r["parent"] == publication["id"])
    );
    observe(&process, json!({"type":"discard", "id":id}));
    let next = observe(&process, options)["session"].as_u64().unwrap();
    assert_ne!(next, id);
    reject(&process, json!({"type":"stop", "id":id}));
    observe(&process, json!({"type":"stop", "id":next}));
    process.finish();
}
