use crate::cargo::Graph;
use crate::{Change, Domain, Kind};
use std::collections::{BTreeMap as Map, BTreeSet as Set};

pub(super) struct Scope {
    pub domains: Set<Domain>,
    pub packages: Set<String>,
    pub semantic: Set<Domain>,
    pub unknown: Map<String, Set<String>>,
    pub escalation: Option<String>,
}

impl Scope {
    pub fn new(changes: &[Change], graph: &Graph, complete: bool) -> Self {
        let mut scope = Self {
            domains: Set::new(),
            packages: Set::new(),
            semantic: Set::new(),
            unknown: Map::new(),
            escalation: complete.then(|| "full mode requested".into()),
        };
        for change in changes {
            scope.add(change, graph);
        }
        for package in &scope.packages {
            for consumer in graph.consumers(package) {
                scope.domains.insert(Domain::Consumer(consumer.into()));
            }
        }
        scope
    }

    pub fn consumers(&self) -> impl Iterator<Item = &String> {
        self.domains.iter().filter_map(|domain| match domain {
            Domain::Consumer(package) => Some(package),
            _ => None,
        })
    }

    pub fn skipped(&self) -> Vec<String> {
        [
            (Domain::Graphics, "graphics: no reachable graphics change"),
            (Domain::Typography, "typography: no reachable text change"),
            (Domain::Http, "http: no reachable transport change"),
            (Domain::Script, "script: no reachable JS engine change"),
        ]
        .into_iter()
        .filter(|(domain, _)| !self.domains.contains(domain))
        .map(|(_, reason)| reason.into())
        .collect()
    }

    fn add(&mut self, change: &Change, graph: &Graph) {
        self.route(&change.path.to_string_lossy(), graph);
        if change.kind == Kind::Renamed
            && let Some(previous) = &change.previous
        {
            self.route(&previous.to_string_lossy(), graph);
        }
    }

    fn route(&mut self, path: &str, graph: &Graph) {
        let fact = Fact(path);
        if fact.infrastructure() {
            self.escalation = Some(format!("workspace infrastructure changed: {path}"));
            self.domains.insert(Domain::Workspace);
            return;
        }
        let package = graph.package(std::path::Path::new(path));
        if let Some(package) = package {
            self.packages.insert(package.into());
            self.domains.insert(Domain::Crate(package.into()));
        }
        if let Some(domain) = fact.capability() {
            self.semantic.insert(domain.clone());
            self.domains.insert(domain);
        } else if !fact.inert() {
            self.unclassified(path, package);
        }
    }

    fn unclassified(&mut self, path: &str, package: Option<&str>) {
        if let Some(package) = package {
            self.unknown
                .entry(package.into())
                .or_default()
                .insert(path.into());
        } else {
            self.escalation = Some(format!("unclassified executable change: {path}"));
            self.domains.insert(Domain::Workspace);
        }
    }
}

struct Fact<'a>(&'a str);

impl Fact<'_> {
    fn capability(&self) -> Option<Domain> {
        if self.0 == "tools/spec/resources/profile.json" {
            return Some(Domain::Profile);
        }
        let prefixes = [
            ("tools/check/", Domain::Tooling),
            ("crates/textengine/", Domain::Typography),
            ("crates/graphics/", Domain::Graphics),
            ("crates/jsengine/", Domain::Script),
            ("crates/jsruntime/", Domain::Script),
        ];
        if let Some((_, domain)) = prefixes
            .iter()
            .find(|(prefix, _)| self.0.starts_with(prefix))
        {
            return Some(domain.clone());
        }
        if self.0.contains("/typography/") {
            return Some(Domain::Typography);
        }
        if self.0.contains("/resource/http") || self.0.ends_with("tests/http.rs") {
            return Some(Domain::Http);
        }
        self.input().then_some(Domain::Input)
    }

    fn input(&self) -> bool {
        let fragments = [
            "/editing/",
            "/platform/device.rs",
            "/platform/src/macos/ime.rs",
            "/platform/src/macos/input.rs",
            "/ipc/session/input.rs",
        ];
        self.0.ends_with("/editing.rs") || fragments.iter().any(|part| self.0.contains(part))
    }

    fn infrastructure(&self) -> bool {
        let files = ["Cargo.toml", "Cargo.lock", "ectropy.toml"];
        let directories = [".cargo/"];
        files.contains(&self.0)
            || directories.iter().any(|prefix| self.0.starts_with(prefix))
            || self.0.ends_with("/build.rs")
    }

    fn inert(&self) -> bool {
        self.0 == "AGENTS.md" || self.0.ends_with(".md")
    }
}
