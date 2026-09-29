use serde_json::{Value, json};
use spec::experiment::{Plan, inspect, run};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "plectron-experiment-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn entry(id: &str) -> Value {
    json!({"id": id, "scenario": "gallery", "size": "small", "backend": "cpu", "runs": 1, "config": {"scenario.count": 6}})
}
#[test]
fn validation() {
    let root = Directory::new();
    for entries in [
        vec![],
        vec![entry("a"), entry("a")],
        vec![entry("../a")],
        vec![entry("a/b")],
        vec![entry("manifest")],
    ] {
        let path = root.write("plan.json", &json!({"version":1,"entries":entries}));
        assert!(Plan::read(&path).is_err());
    }
    let path = root.write("plan.json", &json!({"version":2,"entries":[entry("a")]}));
    assert!(Plan::read(&path).is_err());
}
#[test]
fn execution() {
    let root = Directory::new();
    let path = root.write(
        "plan.json",
        &json!({"version":1,"entries":[entry("gallery")]}),
    );
    let output = root.0.join("first");
    let manifest = run(
        &path,
        &output,
        ("test", "test"),
        std::env::current_exe().unwrap(),
    )
    .unwrap();
    assert!(manifest.valid());
    assert_ne!(manifest.fingerprint, 0);
    assert!(inspect(&output).valid());
    assert!(
        run(
            &path,
            &output,
            ("test", "test"),
            std::env::current_exe().unwrap()
        )
        .is_err()
    );
    let mut baseline = entry("gallery");
    baseline["baseline"] = json!("first/gallery.json");
    baseline["rules"] = json!({"same":["shaping","painting"]});
    let path = root.write("plan.json", &json!({"version":1,"entries":[baseline]}));
    let output = root.0.join("second");
    let manifest = run(
        &path,
        &output,
        ("test", "test"),
        std::env::current_exe().unwrap(),
    )
    .unwrap();
    assert!(manifest.valid());
    assert!(inspect(&output).valid());
    let original = serde_json::to_value(&manifest).unwrap();
    let mut interrupted = original.clone();
    interrupted["entries"] = json!([]);
    root.write("second/manifest.json", &interrupted);
    assert!(!inspect(&output).valid());
    let mut incomplete = original.clone();
    incomplete["entries"][0]["comparison"] = Value::Null;
    root.write("second/manifest.json", &incomplete);
    assert!(!inspect(&output).valid());
    root.write("second/manifest.json", &original);
    std::fs::remove_file(output.join("gallery.json")).unwrap();
    assert!(!inspect(&output).valid());
}
#[test]
fn failures() {
    let root = Directory::new();
    let mut missing = entry("missing");
    missing["scenario"] = json!("unknown");
    let mut baseline = entry("baseline");
    baseline["baseline"] = json!("absent.json");
    let path = root.write(
        "plan.json",
        &json!({"version":1,"entries":[missing,baseline,entry("ok")]}),
    );
    let output = root.0.join("batch");
    let manifest = run(
        &path,
        &output,
        ("test", "test"),
        std::env::current_exe().unwrap(),
    )
    .unwrap();
    assert!(!manifest.valid());
    assert_eq!(manifest.entries.len(), 3);
    assert!(manifest.entries[0].report.is_none());
    assert!(manifest.entries[1].report.is_some());
    assert!(manifest.entries[1].error.is_some());
    assert!(manifest.entries[2].summary.as_ref().unwrap().valid());
    assert!(!inspect(&output).valid());
}
#[test]
fn commands() {
    let root = Directory::new();
    let a = root.write("a.json", &super::fixture());
    let mut changed = super::fixture();
    changed["runs"][1]["stages"][3]["rendering"]["checksum"] = json!(999);
    let b = root.write("b.json", &changed);
    let command = |args: &[&std::ffi::OsStr]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_spec"))
            .args(args)
            .output()
            .unwrap()
    };
    let output = command(&["summarize".as_ref(), a.as_os_str()]);
    assert!(output.status.success());
    assert!(serde_json::from_slice::<Value>(&output.stdout).unwrap()["reports"].is_object());
    let output = command(&["compare".as_ref(), a.as_os_str(), a.as_os_str()]);
    assert!(output.status.success());
    let output = command(&["compare".as_ref(), a.as_os_str(), b.as_os_str()]);
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["passed"],
        false
    );
    let output = command(&[
        "compare".as_ref(),
        a.as_os_str(),
        b.as_os_str(),
        "--text".as_ref(),
    ]);
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("comparison: failed | compatible: true"));
    assert!(text.contains("changed"));
    let output = command(&["summarize".as_ref(), a.as_os_str(), "--text".as_ref()]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("runs 2/2 | milliseconds"));
    assert!(text.contains("10.000"));
    let steady = text
        .lines()
        .find(|line| line.starts_with("steady"))
        .unwrap();
    assert!(steady.contains("0/2") && steady.contains("--"));
    let plan = root.write(
        "plan.json",
        &json!({"version":1,"entries":[entry("gallery")]}),
    );
    let destination = root.0.join("output");
    let output = command(&[
        "experiment".as_ref(),
        plan.as_os_str(),
        "--output".as_ref(),
        destination.as_os_str(),
    ]);
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["entries"][0]["id"],
        "gallery"
    );
    let progress = String::from_utf8(output.stderr).unwrap();
    assert!(progress.contains("[1/1] gallery") && progress.contains("gallery: passed"));
}
