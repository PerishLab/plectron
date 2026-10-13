use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

struct Serving(Child);

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().unwrap();
    }
}

fn start(directory: &Path, root: Option<&Path>) -> (Serving, serde_json::Value) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_plectron-serve"));
    command.current_dir(directory).stdout(Stdio::piped());
    if let Some(root) = root {
        command.arg("--root").arg(root);
    }
    let mut server = Serving(command.spawn().unwrap());
    let mut ready = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    (server, serde_json::from_str(&ready).unwrap())
}

#[test]
fn runtime() {
    let repository = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repository.path())
        .status()
        .unwrap();
    assert!(status.success());
    let root = repository.path().join("apps");
    let nested = repository.path().join("nested");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&nested).unwrap();
    let (_server, ready) = start(&nested, None);
    assert_eq!(
        Path::new(ready["root"].as_str().unwrap()),
        root.canonicalize().unwrap()
    );
}

#[test]
fn explicit() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("assets");
    std::fs::create_dir(&root).unwrap();
    let (_server, ready) = start(directory.path(), Some(&root));
    assert_eq!(
        Path::new(ready["root"].as_str().unwrap()),
        root.canonicalize().unwrap()
    );
}
