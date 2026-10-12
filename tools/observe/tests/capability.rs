#[cfg(not(target_os = "macos"))]
#[test]
fn recording() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("output");
    let config = plectron_observe::native::Config {
        plan: directory.path().join("missing.json"),
        binary: directory.path().join("missing"),
        output: output.clone(),
        seconds: 1,
        limit: None,
    };
    assert!(
        plectron_observe::native::record(&config)
            .unwrap_err()
            .contains("requires macOS")
    );
    assert!(!output.exists());
}

#[cfg(not(unix))]
#[test]
fn transport() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("output");
    let config = plectron_observe::ipc::Config {
        baseline: directory.path().join("missing"),
        internal: directory.path().join("missing"),
        candidate: directory.path().join("missing"),
        output: output.clone(),
        rounds: 1,
        runs: 1,
    };
    assert!(
        plectron_observe::ipc::run(&config)
            .unwrap_err()
            .contains("Unix control transport")
    );
    assert!(!output.exists());
}
