use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

pub fn prepare() -> PathBuf {
    static READY: OnceLock<PathBuf> = OnceLock::new();
    READY
        .get_or_init(|| {
            let root = root();
            let output = Command::new("pnpm")
                .args(["install", "--frozen-lockfile"])
                .current_dir(&root)
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
