use plectron_check::{Cost, Gate, Plan, execute};
use plumb::observation::{Declaration, Run};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn root() -> std::path::PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

fn command(output: &[u8]) -> String {
    String::from_utf8_lossy(output)
        .lines()
        .filter(|line| !line.starts_with("locus engine diagnostic"))
        .collect::<Vec<_>>()
        .join("\n")
}

const CONFIG: &str = "[locus]\nenabled = true\nendpoint = 'http://127.0.0.1:0'\n";

fn records(buffer: &Path) -> Vec<Value> {
    let mut records = Vec::new();
    for entry in std::fs::read_dir(buffer).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name == "active.jsonl" || name.starts_with("sealed-") {
            let content = std::fs::read_to_string(&path).unwrap();
            records.extend(
                content
                    .lines()
                    .map(|line| serde_json::from_str::<Value>(line).unwrap()),
            );
        }
    }
    records
}

fn finish(buffer: &Path) -> Value {
    records(buffer)
        .into_iter()
        .find(|atom| atom["payload"]["event"] == "cli.finish")
        .unwrap()
}

fn verify(program: &str) -> (Value, bool) {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("config.toml");
    std::fs::write(&file, CONFIG).unwrap();
    let buffer = home.path().join("state/locus");
    let declaration = Declaration {
        producer: "plectron",
        file: &file,
        buffer: buffer.clone(),
    };
    let run = Run::start(declaration, "verify").unwrap();
    let mut plan = Plan {
        mode: "run".into(),
        changes: Vec::new(),
        domains: Vec::new(),
        gates: vec![Gate {
            id: "format".into(),
            command: vec![program.into()],
            cost: Cost::Small,
            reasons: Vec::new(),
            result: None,
        }],
        skipped: Vec::new(),
        escalation: None,
    };
    let passed = execute(&root(), &mut plan).is_ok();
    run.finish(i32::from(!passed));
    (finish(&buffer), passed)
}

#[test]
fn fault() {
    let (record, passed) = verify("false");
    assert!(!passed);
    assert_eq!(record["payload"]["code"], 1);
    assert_eq!(record["context"]["plectron.fault"], "format");
    assert_eq!(record["context"]["plectron.command"], "verify");
    let (record, passed) = verify("true");
    assert!(passed);
    assert_eq!(record["payload"]["code"], 0);
    assert!(record["context"]["plectron.fault"].is_null());
}

fn invoke(home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plectron-check"))
        .args(arguments)
        .current_dir(root())
        .env("HOME", home)
        .output()
        .unwrap()
}

#[test]
fn parity() {
    let enabled = tempfile::tempdir().unwrap();
    let seat = enabled.path().join(".plectron");
    std::fs::create_dir_all(&seat).unwrap();
    std::fs::write(seat.join("config.toml"), CONFIG).unwrap();
    let disabled = tempfile::tempdir().unwrap();
    for arguments in [
        &["plan", "--range", "HEAD..HEAD", "--json"][..],
        &["unknown"][..],
    ] {
        let on = invoke(enabled.path(), arguments);
        let off = invoke(disabled.path(), arguments);
        assert_eq!(on.status.code(), off.status.code(), "{arguments:?}");
        assert_eq!(on.stdout, off.stdout, "{arguments:?}");
        assert_eq!(command(&on.stderr), command(&off.stderr), "{arguments:?}");
    }
    let atoms = records(&seat.join("state/locus"));
    assert!(
        atoms
            .iter()
            .any(|atom| atom["payload"]["event"] == "cli.finish")
    );
}
