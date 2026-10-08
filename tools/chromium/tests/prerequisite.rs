use plectron_chromium::{prerequisite, read};
use std::process::{Command, Output};

fn output(success: bool, bytes: &[u8]) -> Output {
    let status = Command::new(env!("CARGO_BIN_EXE_plectron-chromium"))
        .arg(if success { "--version" } else { "absent" })
        .output()
        .unwrap()
        .status;
    Output {
        status,
        stdout: bytes.to_vec(),
        stderr: vec![255, 1],
    }
}

#[test]
fn identity() {
    let root = tempfile::tempdir().unwrap();
    let receipt = prerequisite::record(root.path(), Ok(output(true, b"  0.1.17\n"))).unwrap();
    assert_eq!(receipt["version"], "0.1.17");
    assert_eq!(receipt["stdout"], serde_json::json!(b"  0.1.17\n"));
    assert_eq!(receipt["stderr"], serde_json::json!([255, 1]));
    assert_eq!(receipt["exit"], 0);
    assert!(receipt["error"].is_null());
    assert_eq!(
        read(&root.path().join("prerequisites.json")).unwrap(),
        receipt
    );
}

#[test]
fn refusal() {
    for result in [
        Err("missing executable".into()),
        Ok(output(false, b"0.1.17")),
        Ok(output(true, b" \n")),
        Ok(output(true, &[255])),
    ] {
        let root = tempfile::tempdir().unwrap();
        assert!(prerequisite::record(root.path(), result).is_err());
        let receipt = read(&root.path().join("prerequisites.json")).unwrap();
        assert!(receipt["version"].is_null());
        assert!(receipt["error"].is_string());
    }
}

#[test]
fn retained() {
    let root = tempfile::tempdir().unwrap();
    prerequisite::record(root.path(), Err("first failure".into())).unwrap_err();
    let path = root.path().join("prerequisites.json");
    let first = std::fs::read(&path).unwrap();
    prerequisite::record(root.path(), Ok(output(true, b"0.1.17"))).unwrap_err();
    assert_eq!(std::fs::read(path).unwrap(), first);
}

#[test]
fn batches() {
    let mut report = serde_json::json!({
        "version": "fixture", "browser": "fixture", "host": "fixture",
        "sources": "fixture", "reset": "fixture", "adaptations": "fixture",
        "differences": "fixture", "endpoint": "fixture", "accounting": "fixture",
        "gpu": {"device": "fixture"}, "revision": "fixture",
    });
    let original = plectron_chromium::reference::identity(&report).unwrap();
    report["prerequisites"] = serde_json::json!({"version": "first"});
    let recorded = plectron_chromium::reference::identity(&report).unwrap();
    assert_ne!(original, recorded);
    report["prerequisites"]["version"] = serde_json::json!("second");
    assert_ne!(
        recorded,
        plectron_chromium::reference::identity(&report).unwrap()
    );
}

#[test]
fn entrypoint() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("capture");
    let result = Command::new(env!("CARGO_BIN_EXE_plectron-chromium"))
        .args(["measure", "--output"])
        .arg(&path)
        .env("PATH", root.path().join("absent"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    let receipt = read(&path.join("prerequisites.json")).unwrap();
    assert!(
        receipt["error"]
            .as_str()
            .unwrap()
            .contains("playwright-cli")
    );
    assert!(receipt["exit"].is_null());
    assert!(receipt["version"].is_null());
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 1);
}
