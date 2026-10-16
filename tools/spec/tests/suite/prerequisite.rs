use std::process::Command;

#[test]
fn generator() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("fonts");
    let result = Command::new(env!("CARGO_BIN_EXE_spec"))
        .args(["font-assets", "--fonttools"])
        .arg(root.path().join("absent"))
        .args([
            "--arabic",
            "absent.ttf",
            "--arabic-source",
            "fixture",
            "--arabic-revision",
            "fixture",
            "--cjk",
            "absent",
            "--cjk-source",
            "fixture",
            "--cjk-revision",
            "fixture",
            "--output",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("required upstream font generator")
    );
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn oracle() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("shapes.json");
    let provenance = root.path().join("provenance.json");
    let result = Command::new(env!("CARGO_BIN_EXE_spec"))
        .args(["shape-oracle", "--hb-shape"])
        .arg(root.path().join("absent"))
        .args(["--source", "fixture", "--revision", "fixture", "--output"])
        .arg(&output)
        .arg("--provenance")
        .arg(&provenance)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("required upstream shaping oracle")
    );
    assert!(!output.exists());
    assert!(!provenance.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
