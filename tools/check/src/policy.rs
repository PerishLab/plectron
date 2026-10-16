mod catalog;
mod scope;

use self::scope::Scope;
use crate::cargo::Graph;
use crate::{Change, Cost, Domain, Gate, Plan};
use std::collections::BTreeMap as Map;

pub fn build(changes: Vec<Change>, graph: &Graph, requested_full: bool) -> Plan {
    let scope = Scope::new(&changes, graph, requested_full);
    if scope.escalation.is_some() {
        return complete(changes, scope);
    }
    affected(changes, scope)
}

fn affected(changes: Vec<Change>, scope: Scope) -> Plan {
    let mut gates = catalog::base();
    if !scope.packages.is_empty() {
        gates.push(packages(&scope));
    }
    let consumers: Vec<_> = scope.consumers().cloned().collect();
    if !consumers.is_empty() {
        gates.push(consumer(consumers));
    }
    for domain in &scope.semantic {
        gates.extend(catalog::gates(domain));
    }
    for (package, paths) in &scope.unknown {
        gates.push(gate(
            format!("test:{package}"),
            vec!["cargo".into(), "test".into(), "-p".into(), package.clone()],
            Cost::Medium,
            &format!("unclassified package change: {}", join(paths)),
        ));
    }
    deduplicate(&mut gates);
    let mut skipped = scope.skipped();
    if scope.domains.contains(&Domain::Input) {
        skipped.extend(catalog::unavailable());
    }
    Plan {
        mode: "affected".into(),
        changes,
        domains: scope.domains.into_iter().collect(),
        gates,
        skipped,
        escalation: None,
    }
}

fn complete(changes: Vec<Change>, scope: Scope) -> Plan {
    Plan {
        mode: "full".into(),
        changes,
        domains: scope.domains.into_iter().collect(),
        gates: catalog::complete(),
        skipped: catalog::unavailable(),
        escalation: scope.escalation,
    }
}

fn packages(scope: &Scope) -> Gate {
    let mut command = vec!["cargo".into(), "clippy".into()];
    for package in &scope.packages {
        command.extend(["-p".into(), package.clone()]);
    }
    command.extend([
        "--all-targets".into(),
        "--locked".into(),
        "--".into(),
        "-D".into(),
        "warnings".into(),
    ]);
    gate("clippy:affected", command, Cost::Medium, "changed crates")
}

fn consumer(packages: Vec<String>) -> Gate {
    let mut command = vec!["cargo".into(), "check".into()];
    for package in packages {
        command.extend(["-p".into(), package]);
    }
    command.push("--locked".into());
    gate(
        "check:consumers",
        command,
        Cost::Medium,
        "direct Cargo consumers",
    )
}

fn gate(
    id: impl Into<std::borrow::Cow<'static, str>>,
    command: Vec<String>,
    cost: Cost,
    reason: &str,
) -> Gate {
    Gate {
        id: id.into(),
        command,
        cost,
        reasons: vec![reason.into()],
        result: None,
    }
}

fn deduplicate(gates: &mut Vec<Gate>) {
    let mut unique = Map::<String, Gate>::new();
    for gate in gates.drain(..) {
        unique
            .entry(gate.id.to_string())
            .and_modify(|current| current.reasons.extend(gate.reasons.clone()))
            .or_insert(gate);
    }
    *gates = unique.into_values().collect();
}

fn join(values: &std::collections::BTreeSet<String>) -> String {
    values.iter().cloned().collect::<Vec<_>>().join(", ")
}
