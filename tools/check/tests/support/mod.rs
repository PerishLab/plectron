use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn root() -> PathBuf {
    let output = plectron_check::command(&std::env::current_dir().unwrap(), "git")
        .expect("git is a declared test prerequisite")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    assert!(output.status.success());
    Path::new(String::from_utf8(output.stdout).unwrap().trim())
        .canonicalize()
        .unwrap()
}

pub fn prepare() -> PathBuf {
    static READY: OnceLock<PathBuf> = OnceLock::new();
    READY
        .get_or_init(|| {
            let root = root();
            let output = plectron_check::command(&root, "pnpm")
                .expect("pnpm is a declared static-test prerequisite")
                .args(["install", "--frozen-lockfile"])
                .output()
                .expect("pnpm is a declared static-test prerequisite");
            assert!(
                output.status.success(),
                "frozen static-test dependency installation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            root
        })
        .clone()
}
