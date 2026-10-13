use serde_json::Value;

mod support;

#[test]
fn workspace() {
    let output = plectron_check::command(&support::root(), "cargo")
        .unwrap()
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let graph: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        graph["workspace_default_members"],
        graph["workspace_members"]
    );
    assert_eq!(graph["workspace_members"].as_array().unwrap().len(), 27);
}

#[test]
fn recursive() {
    let root = support::prepare();
    let output = plectron_check::command(&root, "pnpm")
        .unwrap()
        .args(["-r", "--if-present", "run", "typecheck"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(
        stdout.contains(". typecheck$ cargo verify types"),
        "{stdout}"
    );
    assert!(
        stdout.contains("10 compiler-derived PT declarations")
            || stderr.contains("10 compiler-derived PT declarations")
    );
    let output = plectron_check::command(&root, "pnpm")
        .unwrap()
        .arg("test")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for package in ["packages/composition test", "packages/reactive test"] {
        assert!(stdout.contains(package), "{stdout}");
    }
    assert!(!stdout.contains(". test$"));
}
