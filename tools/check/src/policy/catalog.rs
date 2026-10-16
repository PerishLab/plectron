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
        gate(
            "test:registry",
            words("cargo test -p fixtures --test registry --locked"),
            Cost::Small,
            "checked application roles, entries and resource dependencies",
        ),
        gate(
            "test:source",
            words("cargo test -p fixtures --test source --locked"),
            Cost::Small,
            "canonical TS/PT and bounded intentional JS sources",
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
        Domain::Script => script(),
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
            "typecheck",
            words("pnpm typecheck"),
            Cost::Medium,
            "selected TypeScript source profile",
        ),
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
    let mut gates = vec![
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
    ];
    if cfg!(target_os = "macos") {
        gates.push(gate(
            "test:input-adapter",
            words("cargo test -p platform --test keyboard"),
            Cost::Small,
            "macOS native input translation without a window",
        ));
    }
    if cfg!(target_os = "linux") {
        gates.push(gate(
            "test:input-adapter",
            words("cargo test -p platform --lib linux::input"),
            Cost::Small,
            "X11 native input translation without a window",
        ));
    }
    gates
}

pub(super) fn unavailable() -> Vec<String> {
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        Vec::new()
    } else {
        vec!["input-adapter: native keyboard translation tests require macOS or Linux; no native input verification on this host".into()]
    }
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

fn script() -> Vec<Gate> {
    vec![
        gate(
            "test:script",
            words("cargo test -p jsengine --locked"),
            Cost::Medium,
            "script domain",
        ),
        gate(
            "test:runtime",
            words("cargo test -p jsruntime --locked"),
            Cost::Small,
            "script composition and source lifetime",
        ),
    ]
}

fn one(id: &'static str, command: &str, cost: Cost, reason: &str) -> Vec<Gate> {
    vec![gate(id, words(command), cost, reason)]
}

fn gate(id: &'static str, command: Vec<String>, cost: Cost, reason: &str) -> Gate {
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
