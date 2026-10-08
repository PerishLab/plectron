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
    let plan = facts("M\ttools/serve/src/main.rs\n");
    assert_eq!(plan.mode, "affected");
    assert!(plan.escalation.is_none());
    assert!(plan.gates.iter().any(|gate| gate.id == "clippy:affected"));
    assert!(
        plan.gates
            .iter()
            .any(|gate| gate.id == "test:plectron-serve")
    );
}

#[test]
fn runtime() {
    for path in [
        "crates/jsruntime/src/source.rs",
        "crates/jsengine/src/fault/source.rs",
    ] {
        let plan = facts(&format!("M\t{path}\n"));
        assert_eq!(plan.mode, "affected");
        for id in ["test:script", "test:runtime"] {
            let gate = plan
                .gates
                .iter()
                .find(|gate| gate.id == id)
                .expect("script and runtime gates");
            assert!(gate.command.iter().any(|argument| argument == "--locked"));
        }
    }
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

#[test]
fn typescript() {
    for path in ["apps/typescript/main.ts", "tsconfig.json", "types/pt.d.ts"] {
        let plan = facts(&format!("M\t{path}\n"));
        assert_eq!(plan.mode, "full");
        let gate = plan
            .gates
            .iter()
            .find(|gate| gate.id == "typecheck")
            .unwrap();
        assert_eq!(gate.command, ["pnpm", "typecheck"]);
    }
}

#[test]
fn registry() {
    for changes in ["M\tapps/registry.json\n", "M\ttools/check/src/policy.rs\n"] {
        let plan = facts(changes);
        let gate = plan
            .gates
            .iter()
            .find(|gate| gate.id == "test:registry")
            .unwrap();
        assert_eq!(
            gate.command,
            [
                "cargo", "test", "-p", "fixtures", "--test", "registry", "--locked"
            ]
        );
    }
}

#[test]
fn source() {
    let plan = facts("M\tpackages/engine/src/index.ts\n");
    let gate = plan
        .gates
        .iter()
        .find(|gate| gate.id == "test:source")
        .unwrap();
    assert_eq!(
        gate.command,
        [
            "cargo", "test", "-p", "fixtures", "--test", "source", "--locked"
        ]
    );
}
