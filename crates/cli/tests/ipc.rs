#![cfg(target_os = "macos")]

mod process;
use process::Process;
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::{fs::PermissionsExt, net::UnixStream};
use std::time::{Duration, Instant};

#[test]
fn lifecycle() {
    let mut process = Process::start();
    assert_eq!(
        std::fs::metadata(&process.directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&process.socket)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let status = process.control(&["status"]);
    assert_eq!(status["pid"], process.child.id());
    assert_eq!(status["mode"], "running");
    assert_eq!(status["windows"], 0);
    let output = process.command(&["ps"]);
    assert!(output.status.success());
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(listed["result"]["Ok"]["pid"], process.child.id());
    assert_eq!(process.evaluate("count = count + 2; count;")["Ok"], "12");
    process.control(&["pause"]);
    assert_eq!(process.control(&["status"])["mode"], "paused");
    assert_eq!(process.control(&["step"])["dispatched"], false);
    process.control(&["resume"]);
    assert_eq!(
        process.control(&["windows"])["entries"],
        serde_json::json!([])
    );
    assert_eq!(
        process.control(&["config", r#"{"type":"inspect"}"#])["entries"],
        serde_json::json!({})
    );
    assert_eq!(
        process.control(&[
            "config",
            r#"{"type":"set","key":"events.scopes","value":-1}"#
        ])["entries"]["events.scopes"],
        -1
    );
    assert_eq!(
        process.control(&["config", r#"{"type":"inspect"}"#])["entries"]["events.scopes"],
        -1
    );
    assert_eq!(
        process.control(&["config", r#"{"type":"remove","key":"events.scopes"}"#])["entries"],
        serde_json::json!({})
    );
    process.finish();
}

#[test]
fn malformed() {
    let mut process = Process::start();
    for input in [
        "garbage\n",
        "{\"version\":2,\"command\":{\"type\":\"shutdown\"}}\n",
        "{\"version\":1,\"command\":{\"type\":\"shutdown\",\"extra\":true}}\n",
        "{\"version\":1,\"command\":{\"type\":\"unknown\"}}\n",
    ] {
        assert!(process.raw(input.as_bytes())["result"]["Err"].is_string());
    }
    let input = vec![b'x'; 65_537];
    assert!(
        process.raw(&input)["result"]["Err"]
            .as_str()
            .unwrap()
            .contains("budget")
    );
    assert_eq!(process.control(&["status"])["mode"], "running");
    process.finish();
}

#[test]
fn deadline() {
    let mut process = Process::start();
    let started = Instant::now();
    assert!(process.raw(b"{")["result"]["Err"].is_string());
    assert!(started.elapsed() >= Duration::from_secs(1));
    assert_eq!(process.control(&["status"])["mode"], "running");
    process.finish();
}

#[test]
fn fragmented() {
    let mut process = Process::start();
    let mut stream = UnixStream::connect(&process.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(b"{\"version\":1,").unwrap();
    std::thread::sleep(Duration::from_millis(50));
    stream
        .write_all(b"\"command\":{\"type\":\"status\"}}\n")
        .unwrap();
    let mut bytes = String::new();
    stream.read_to_string(&mut bytes).unwrap();
    let response: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(response["result"]["Ok"]["pid"], process.child.id());
    process.finish();
}

#[test]
fn startup() {
    let mut process = Process::start();
    let script = process.root.join("broken.js");
    std::fs::write(&script, "missing();").unwrap();
    let output = process.command(&["run", script.to_str().unwrap()]);
    assert!(!output.status.success());
    assert_eq!(std::fs::read_dir(&process.directory).unwrap().count(), 1);
    std::fs::write(&script, "1 + 2;").unwrap();
    let output = process.command(&["run", script.to_str().unwrap()]);
    assert!(output.status.success());
    assert_eq!(std::fs::read_dir(&process.directory).unwrap().count(), 1);
    process.finish();
}

#[test]
fn failure() {
    let mut process = Process::start();
    let pid = process.child.id().to_string();
    assert!(process.evaluate("missing();")["Err"].is_string());
    {
        let output = process.command(&["ctl", &pid, "close", "999"]);
        assert!(!output.status.success());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(response["result"]["Err"].is_string());
    }
    assert_eq!(process.evaluate("count;")["Ok"], "10");
    process.finish();
}

#[test]
fn large() {
    let mut process = Process::start();
    let value = "x".repeat(24_000);
    let source = format!("'{value}';");
    assert_eq!(process.evaluate(&source)["Ok"], value);
    process.finish();
}

#[test]
fn services() {
    let mut process = Process::start();
    let source = format!(
        "let server = platform.serve({:?}); server.scheme('app://demo/');",
        process.root.to_str().unwrap()
    );
    assert!(process.evaluate(&source)["Ok"].is_string());
    let snapshot = process.control(&["services"]);
    let address = snapshot["entries"][0]["address"].as_str().unwrap();
    let endpoint = address.strip_prefix("http://").unwrap().to_string();
    assert_eq!(snapshot["entries"][0]["schemes"][0], "app://demo/");
    process.finish();
    assert!(std::net::TcpStream::connect(endpoint).is_err());
}

#[test]
fn held() {
    let mut process = Process::start();
    let held = process.control(&["hold"])["job"].clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = process.control(&["control"]);
        if state["snapshot"]["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|job| job["id"] == held && job["phase"] == "held")
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let queued = process.control(&["eval", "count = count + 1; count;"])["job"].clone();
    assert_eq!(process.control(&["status"])["mode"], "running");
    let state = process.control(&["control"]);
    assert!(
        state["snapshot"]["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|job| job["id"] == queued && job["phase"] == "queued")
    );
    process.control(&["unhold"]);
    assert_eq!(process.completed(&queued)["Ok"], "11");
    process.control(&["hold"]);
    process.finish();
}

#[test]
fn caches() {
    let mut process = Process::start();
    let pid = process.child.id().to_string();
    let output = process.command(&["ctl", &pid, "cache", "1", "1", r#"{"action":"clear"}"#]);
    assert!(!output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["result"]["Err"]
            .as_str()
            .unwrap()
            .contains("window")
    );
    let invalid = process.command(&["ctl", &pid, "cache", "1", "1", r#"{"action":"unknown"}"#]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unknown variant"));
    process.finish();
}

#[test]
fn isolation() {
    let mut process = Process::start();
    let mut slow = UnixStream::connect(&process.socket).unwrap();
    slow.write_all(b"{").unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let started = Instant::now();
    assert_eq!(process.control(&["status"])["mode"], "running");
    assert!(started.elapsed() < Duration::from_secs(1));
    process.finish();
}
