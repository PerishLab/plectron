use std::process::Command;

fn plectron(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_plectron"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn unknown() {
    for option in ["--versio", "-x"] {
        let output = plectron(&[option]);
        assert!(!output.status.success(), "{option} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(option) && stderr.contains("unknown option"),
            "stderr for {option}: {stderr}"
        );
        assert!(
            !stderr.contains("No such file or directory"),
            "stderr for {option}: {stderr}"
        );
    }
}

#[test]
fn script() {
    let output = plectron(&["definitely-missing-151.js"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("unknown option"), "stderr: {stderr}");
}

#[test]
fn run() {
    let output = plectron(&["run", "definitely-missing-151.js"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("unknown option"), "stderr: {stderr}");
}
