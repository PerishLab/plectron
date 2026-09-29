use plectron_check::{Domain, Source, plan};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

static FILES: AtomicUsize = AtomicUsize::new(0);

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn facts(text: &str) -> plectron_check::Plan {
    let index = FILES.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("plectron-check-{}-{index}", std::process::id()));
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
    let result = plan(&root(), &Source::File(path.clone()), false).unwrap();
    std::fs::remove_file(path).unwrap();
    result
}

#[test]
fn composition() {
    let gates = facts(
        "M\tcrates/webengine/src/editing/composition.rs\nM\ttools/spec/resources/profile.json\n",
    )
    .gates
    .into_iter()
    .map(|gate| gate.id)
    .collect::<Vec<_>>();
    for expected in [
        "test:input-dom",
        "test:input-web",
        "test:input-pointer",
        "test:input-composition",
        "test:input-editing",
        "test:input-adapter",
        "test:profile-capture",
        "test:profile-integrity",
    ] {
        assert!(gates.iter().any(|gate| gate == expected), "{expected}");
    }
    assert!(!gates.iter().any(|gate| gate == "test:graphics"));
}

#[test]
fn adapter() {
    let plan = facts("M\tcrates/platform/src/macos/ime.rs\n");
    assert!(plan.domains.contains(&Domain::Input));
    assert!(
        plan.gates
            .iter()
            .any(|gate| gate.id == "test:input-adapter")
    );
    assert_eq!(plan.mode, "affected");
}

#[test]
fn focus() {
    let plan = facts("M\tapps/todo/main.js\nM\tcrates/webengine/src/editing/composition.rs\n");
    assert!(plan.domains.contains(&Domain::Input));
    assert_eq!(plan.mode, "full");
    assert!(plan.escalation.unwrap().contains("apps/todo/main.js"));
}

#[test]
fn infrastructure() {
    let plan = facts("M\tCargo.toml\n");
    assert_eq!(plan.mode, "full");
    assert!(plan.gates.iter().any(|gate| gate.id == "test:graphics"));
}

#[test]
fn http() {
    let plan = facts("M\tcrates/lib/src/resource/http/client.rs\n");
    assert!(plan.domains.contains(&Domain::Http));
    assert!(plan.gates.iter().any(|gate| gate.id == "test:http"));
    assert!(!plan.gates.iter().any(|gate| gate.id == "test:graphics"));
}

#[test]
fn unknown() {
    let plan = facts("M\tcrates/config/src/value.rs\n");
    assert!(plan.gates.iter().any(|gate| gate.id == "test:config"));
    assert!(
        !plan
            .gates
            .iter()
            .any(|gate| gate.id == "test:plectron-check")
    );
}

#[test]
fn mixed() {
    let plan = facts(
        "M\tcrates/webengine/src/editing/composition.rs\nM\tcrates/webengine/src/page/demand.rs\n",
    );
    assert!(plan.gates.iter().any(|gate| gate.id == "test:input-web"));
    assert!(plan.gates.iter().any(|gate| gate.id == "test:webengine"));
}

#[test]
fn script() {
    let plan = facts("M\ttools/serve/serve.py\n");
    assert_eq!(plan.mode, "full");
    assert!(plan.escalation.unwrap().contains("unclassified"));
}

#[test]
fn renamed() {
    let plan = facts(
        "R100\tcrates/webengine/src/editing/composition.rs\tcrates/webengine/src/composition.rs\n",
    );
    assert!(plan.domains.contains(&Domain::Input));
    assert!(plan.gates.iter().any(|gate| gate.id == "test:input-web"));
}

#[test]
fn manifest() {
    let plan = facts("M\tCargo.toml\n");
    assert_eq!(plan.mode, "full");
    assert!(plan.escalation.unwrap().contains("infrastructure"));
}
