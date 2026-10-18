mod graph;
mod markup;

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Canonical,
    Language,
    Upstream,
    Browser,
    Driver,
    Harness,
}

pub struct Audit {
    pub files: BTreeMap<String, Role>,
    pub graphs: usize,
    pub exports: usize,
    pub scripts: usize,
}

pub fn role(path: &str) -> Result<Role, String> {
    match path {
        "apps/list/scenario.js" => Ok(Role::Driver),
        "packages/reactive/tests/engine.js"
        | "packages/reactive/tests/gc.js"
        | "packages/reactive/tests/errors.js"
        | "packages/reactive/tests/boundary.js" => Ok(Role::Language),
        "tools/chromium/paired.js" | "tools/chromium/cards.js" => Ok(Role::Browser),
        "tools/spec/resources/harness.js" => Ok(Role::Harness),
        path if path.starts_with("tools/spec/resources/upstream/")
            && !path
                .strip_prefix("tools/spec/resources/upstream/")
                .unwrap()
                .contains('/')
            && path.ends_with(".js") =>
        {
            Ok(Role::Upstream)
        }
        path if path.ends_with(".ts") || path.ends_with(".pt") => Ok(Role::Canonical),
        _ => Err(format!("unclassified execution source: {path}")),
    }
}

pub fn audit(root: &Path) -> Result<Audit, String> {
    let registry = crate::registry::Registry::open(root)?;
    let mut files = Vec::new();
    for directory in ["apps", "packages", "crates", "tools"] {
        collect(root, &root.join(directory), &mut files)?;
    }
    let mut result = Audit {
        files: BTreeMap::new(),
        graphs: 0,
        exports: 0,
        scripts: 0,
    };
    for path in files {
        if matches!(
            Path::new(&path)
                .extension()
                .and_then(|value| value.to_str()),
            Some("js" | "mjs" | "cjs" | "ts" | "pt")
        ) {
            let role = role(&path)?;
            result.files.insert(path.clone(), role);
            if Fact(&path).canonical() {
                graph::link(root, &path).map_err(|error| format!("{path}: {error}"))?;
                result.graphs += 1;
            }
        }
        if Fact(&path).production() {
            let source = std::fs::read_to_string(root.join(&path))
                .map_err(|error| format!("read {path}: {error}"))?;
            stale(&source)?;
            retired(&path, &source, &registry)?;
        }
    }
    for entry in std::fs::read_dir(root.join("packages")).map_err(error)? {
        let path = entry.map_err(error)?.path();
        if !path.is_dir() {
            continue;
        }
        let relative = relative(root, &path)?;
        let manifest = std::fs::read_to_string(path.join("package.json")).map_err(error)?;
        let package =
            project::Package::parse(project::Module::new(&relative).map_err(error)?, &manifest)
                .map_err(error)?;
        for module in package.exports.values() {
            export(module)?;
            graph::link(root, module.path())?;
            result.exports += 1;
        }
    }
    for app in registry.applications.values() {
        if let Some(control) = &app.control {
            graph::link(root, &format!("apps/{control}"))
                .map_err(|error| format!("control {control}: {error}"))?;
        }
        if let Some(content) = &app.content {
            let path = format!("apps/{content}");
            let source = std::fs::read_to_string(root.join(&path)).map_err(error)?;
            for entry in scripts(&Module::new(path).map_err(error)?, &source)? {
                graph::link(root, entry.path())?;
                result.scripts += 1;
            }
        }
    }
    Ok(result)
}

pub use markup::scripts;

fn retired(path: &str, source: &str, registry: &crate::registry::Registry) -> Result<(), String> {
    for name in registry.applications.keys() {
        for entry in ["main", "start", "model"] {
            let retired = format!("apps/{name}/{entry}.js");
            if source.contains(&retired) {
                return Err(format!("stale product source reference: {path}: {retired}"));
            }
        }
    }
    Ok(())
}

use project::Module;

pub fn export(module: &project::Module) -> Result<(), String> {
    if !matches!(
        module.kind().map_err(error)?,
        project::Kind::Ts | project::Kind::Pt
    ) {
        return Err(format!("package export is not canonical TS/PT: {module}"));
    }
    Ok(())
}

pub fn stale(source: &str) -> Result<(), String> {
    for retired in [
        ["exam", "ples/"].concat(),
        ["@plectron/", "design"].concat(),
    ] {
        if source.contains(&retired) {
            return Err(format!("retired product path: {retired}"));
        }
    }
    Ok(())
}

struct Fact<'a>(&'a str);

impl Fact<'_> {
    fn canonical(&self) -> bool {
        let owned = self.0.starts_with("apps/")
            || (self.0.starts_with("packages/") && self.0.contains("/src/"));
        owned
            && matches!(
                Path::new(self.0)
                    .extension()
                    .and_then(|value| value.to_str()),
                Some("ts" | "pt")
            )
    }

    fn production(&self) -> bool {
        let owned = self.0.starts_with("crates/") || self.0.starts_with("tools/");
        owned && self.0.contains("/src/") && self.0.ends_with(".rs")
    }
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(error)? {
        let path = entry.map_err(error)?.path();
        if path.is_dir() {
            if matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("target" | "node_modules" | ".git")
            ) {
                continue;
            }
            collect(root, &path, files)?;
        } else {
            files.push(relative(root, &path)?);
        }
    }
    Ok(())
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(crate) fn relative(root: &Path, path: &Path) -> Result<String, String> {
    let path = path.strip_prefix(root).map_err(error)?;
    project::Module::file(path)
        .map(|module| module.path().to_owned())
        .map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative() {
        let root = Path::new("workspace");
        let path = root.join("packages").join("含 空格").join("nested/main.pt");
        assert_eq!(
            super::relative(root, &path).unwrap(),
            "packages/含 空格/nested/main.pt"
        );
        assert!(super::relative(root, Path::new("elsewhere/main.pt")).is_err());
    }
}
