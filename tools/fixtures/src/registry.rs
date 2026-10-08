mod references;

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Representative,
    Conformance,
    Workload,
    Resource,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumer {
    pub role: Role,
    pub path: String,
    pub reference: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub roles: BTreeSet<Role>,
    pub control: Option<String>,
    pub content: Option<String>,
    pub dependencies: BTreeSet<String>,
    pub consumers: Vec<Consumer>,
    pub workload: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub applications: BTreeMap<String, Application>,
}

impl Registry {
    pub fn open(root: &Path) -> Result<Self, String> {
        let source = std::fs::read(root.join("apps/registry.json")).map_err(error)?;
        let registry: Self = serde_json::from_slice(&source).map_err(error)?;
        registry.validate(root)?;
        Ok(registry)
    }

    pub fn validate(&self, root: &Path) -> Result<(), String> {
        let directory = root.join("apps");
        let names = std::fs::read_dir(&directory)
            .map_err(error)?
            .map(|entry| {
                let entry = entry.map_err(error)?;
                Ok(entry
                    .file_type()
                    .map_err(error)?
                    .is_dir()
                    .then(|| entry.file_name().to_string_lossy().into_owned()))
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        if names != self.applications.keys().cloned().collect() {
            return Err(
                "application registry does not cover exactly the maintained directories".into(),
            );
        }
        for (name, app) in &self.applications {
            self.application(root, name, app)?;
        }
        self.catalog(root)
    }

    pub fn workload(&self, id: &str, entry: &str, launcher: &str) -> Result<(), String> {
        let app = self
            .applications
            .values()
            .find(|app| app.workload.as_deref() == Some(id))
            .ok_or_else(|| format!("unregistered workload: {id}"))?;
        if !app.roles.contains(&Role::Workload)
            || app.content.as_deref() != Some(entry)
            || app.control.as_deref() != Some(launcher)
        {
            return Err(format!("workload entries disagree with registry: {id}"));
        }
        Ok(())
    }

    fn application(&self, root: &Path, name: &str, app: &Application) -> Result<(), String> {
        if app.roles.is_empty() {
            return Err(format!("missing application roles: {name}"));
        }
        let directory = root.join("apps");
        for entry in [&app.control, &app.content].into_iter().flatten() {
            file(&directory, entry)?;
            let owner = entry.split('/').next().unwrap();
            if owner != name && !app.dependencies.contains(owner) {
                return Err(format!("undeclared entry dependency: {name} -> {owner}"));
            }
        }
        let control = directory.join(name).join("start.ts");
        let expected = control.is_file().then(|| format!("{name}/start.ts"));
        if app.control != expected {
            return Err(format!("control entry disagrees with source: {name}"));
        }
        let manifest = directory.join(name).join("plectron.toml");
        if manifest.is_file() {
            let source = std::fs::read_to_string(manifest).map_err(error)?;
            let entry = app
                .control
                .as_deref()
                .ok_or_else(|| format!("missing control: {name}"))?;
            if !source.contains(&format!(
                "entry = {:?}",
                entry
                    .strip_prefix(&format!("{name}/"))
                    .ok_or("foreign control entry")?
            )) {
                return Err(format!("manifest entry disagrees with registry: {name}"));
            }
        }
        let content = directory.join(name).join("index.html");
        if content.is_file() && app.content.as_deref() != Some(&format!("{name}/index.html")) {
            return Err(format!("content entry disagrees with source: {name}"));
        }
        for role in &app.roles {
            if !app.consumers.iter().any(|consumer| &consumer.role == role) {
                return Err(format!("role has no consumer: {name} {role:?}"));
            }
        }
        for consumer in &app.consumers {
            file(root, &consumer.path)?;
            let source = std::fs::read_to_string(root.join(&consumer.path)).map_err(error)?;
            if !app.roles.contains(&consumer.role)
                || consumer.reference.is_empty()
                || !source.contains(&consumer.reference)
            {
                return Err(format!("consumer reference disagrees with source: {name}"));
            }
        }
        let observed = references::inspect(&directory, name)?;
        let expected = if observed.content.is_empty() {
            content
                .is_file()
                .then(|| format!("{name}/index.html"))
                .into_iter()
                .collect()
        } else {
            observed.content
        };
        if expected != app.content.iter().cloned().collect::<BTreeSet<_>>() {
            return Err(format!("loaded content disagrees with registry: {name}"));
        }
        if observed.dependencies != app.dependencies {
            return Err(format!(
                "resource dependencies disagree: {name}: declared {:?}, observed {:?}",
                app.dependencies, observed.dependencies
            ));
        }
        for dependency in &app.dependencies {
            if dependency == name || !self.applications.contains_key(dependency) {
                return Err(format!(
                    "unknown application dependency: {name} -> {dependency}"
                ));
            }
        }
        Ok(())
    }

    fn catalog(&self, root: &Path) -> Result<(), String> {
        let source = std::fs::read(root.join("apps/catalog.json")).map_err(error)?;
        let cases: Vec<serde_json::Value> = serde_json::from_slice(&source).map_err(error)?;
        let mut ids = BTreeSet::new();
        for case in cases {
            let id = case["id"]
                .as_str()
                .ok_or("catalog identity is not a string")?;
            if !ids.insert(id.to_owned()) {
                return Err(format!("duplicate catalog workload: {id}"));
            }
            self.workload(
                id,
                case["entry"].as_str().ok_or("missing catalog entry")?,
                case["launcher"]
                    .as_str()
                    .ok_or("missing catalog launcher")?,
            )?;
        }
        let mut declared = BTreeSet::new();
        for app in self.applications.values() {
            if let Some(id) = &app.workload
                && !declared.insert(id.clone())
            {
                return Err(format!("duplicate registered workload: {id}"));
            }
        }
        if declared != ids {
            return Err("registered workloads disagree with catalog".into());
        }
        Ok(())
    }
}

fn file(root: &Path, name: &str) -> Result<(), String> {
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("registry path must be relative: {name}"));
    }
    if !root.join(name).is_file() {
        return Err(format!("missing registry file: {name}"));
    }
    Ok(())
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
