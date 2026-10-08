use serde_json::Value;
use std::process::Command;

mod support;

#[test]
fn workspace() {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
        .current_dir(support::root())
        .output()
        .unwrap();
    assert!(output.status.success());
    let graph: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        graph["workspace_default_members"],
        graph["workspace_members"]
    );
    assert_eq!(graph["workspace_members"].as_array().unwrap().len(), 26);
}

#[test]
fn recursive() {
    let root = support::prepare();
    let output = Command::new("pnpm")
        .args(["-r", "--if-present", "run", "typecheck"])
        .current_dir(&root)
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
    let output = Command::new("pnpm")
        .arg("test")
        .current_dir(root)
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
