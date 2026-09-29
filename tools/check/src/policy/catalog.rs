use crate::{Cost, Domain, Gate};

pub(super) fn base() -> Vec<Gate> {
    vec![
        gate(
            "format",
            words("cargo fmt --all -- --check"),
            Cost::Small,
            "all source changes",
        ),
        gate(
            "ectropy",
            words("ectropy ."),
            Cost::Small,
            "all source changes",
        ),
    ]
}

pub(super) fn gates(domain: &Domain) -> Vec<Gate> {
    match domain {
        Domain::Input => input(),
        Domain::Profile => profile(),
        Domain::Graphics => one(
            "test:graphics",
            "cargo test -p graphics",
            Cost::Medium,
            "graphics domain",
        ),
        Domain::Typography => one(
            "test:typography",
            "cargo test -p textengine",
            Cost::Medium,
            "typography domain",
        ),
        Domain::Script => one(
            "test:script",
            "cargo test -p jsengine",
            Cost::Medium,
            "script domain",
        ),
        Domain::Http => one(
            "test:http",
            "cargo test -p plectron --test http",
            Cost::Small,
            "HTTP transport",
        ),
        Domain::Tooling => one(
            "test:check",
            "cargo test -p plectron-check",
            Cost::Small,
            "validation tooling",
        ),
        _ => Vec::new(),
    }
}

pub(super) fn complete() -> Vec<Gate> {
    let mut gates = base();
    gates.extend([
        gate(
            "test:chromium",
            words("cargo test -p plectron-chromium --locked"),
            Cost::Small,
            "full mode",
        ),
        gate(
            "test:observe",
            words("cargo test -p plectron-observe --locked"),
            Cost::Small,
            "full mode",
        ),
        gate(
            "clippy:workspace",
            words("cargo clippy --workspace --all-targets --locked -- -D warnings"),
            Cost::Large,
            "full mode",
        ),
        gate(
            "test:workspace",
            words("cargo test --workspace --exclude graphics --locked"),
            Cost::Large,
            "full mode",
        ),
        gate(
            "test:graphics",
            words("cargo test -p graphics --locked"),
            Cost::Large,
            "full mode",
        ),
    ]);
    gates
}

fn input() -> Vec<Gate> {
    vec![
        gate(
            "test:input-dom",
            words("cargo test -p domengine --test update journal::projection"),
            Cost::Small,
            "input model",
        ),
        gate(
            "test:input-web",
            words("cargo test -p webengine --test conformance editing::composition"),
            Cost::Small,
            "input routing",
        ),
        gate(
            "test:input-pointer",
            words("cargo test -p plectron --test conformance web::transitions::pointer"),
            Cost::Small,
            "virtual pointer source",
        ),
        gate(
            "test:input-composition",
            words(
                "cargo test -p plectron --test conformance applications::list::composition::virtualized",
            ),
            Cost::Small,
            "virtual composition source",
        ),
        gate(
            "test:input-editing",
            words("cargo test -p plectron --test conformance applications::todo::typing"),
            Cost::Small,
            "virtual keyboard source",
        ),
        gate(
            "test:input-adapter",
            words("cargo test -p platform --test keyboard"),
            Cost::Small,
            "native input translation without a window",
        ),
    ]
}

fn profile() -> Vec<Gate> {
    vec![
        gate(
            "test:profile-capture",
            words("cargo test -p spec --test capture"),
            Cost::Small,
            "capability profile",
        ),
        gate(
            "test:profile-integrity",
            words("cargo test -p spec --test conformance"),
            Cost::Small,
            "capability profile",
        ),
    ]
}

fn one(id: &str, command: &str, cost: Cost, reason: &str) -> Vec<Gate> {
    vec![gate(id, words(command), cost, reason)]
}

fn gate(id: &str, command: Vec<String>, cost: Cost, reason: &str) -> Gate {
    Gate {
        id: id.into(),
        command,
        cost,
        reasons: vec![reason.into()],
        result: None,
    }
}

fn words(command: &str) -> Vec<String> {
    command.split_whitespace().map(str::to_string).collect()
}
