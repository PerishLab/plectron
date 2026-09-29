use crate::{Artifact, Fault};
use project::{Files, Import, Module, Package, Provider, Request, Resolve, Resolver, Span};
use std::path::Path;

pub fn build(entry: &Path) -> Result<Artifact, Fault> {
    link(entry, false)
}

pub fn script(entry: &Path) -> Result<Artifact, Fault> {
    link(entry, true)
}

fn link(entry: &Path, script: bool) -> Result<Artifact, Fault> {
    let entry = entry.canonicalize().map_err(error)?;
    let root = entry
        .ancestors()
        .find(|path| path.join("pnpm-workspace.yaml").is_file())
        .ok_or_else(|| Fault("build entry is outside a pnpm workspace".into()))?;
    let path = entry
        .strip_prefix(root)
        .map_err(|failure| Fault(failure.to_string()))?
        .to_str()
        .ok_or_else(|| Fault("entry path is not UTF-8".into()))?;
    let entry = Module::new(path).map_err(fault)?;
    let files = Files::new(root).map_err(error)?;
    let resolver = Resolver::new(packages(root)?).map_err(fault)?;
    let read = |path: &str| {
        let module = Module::new(path).map_err(fault)?;
        files
            .read(Request { module })
            .map(|source| source.text)
            .map_err(diagnostic)
    };
    let resolve = |from: &str, specifier: &str| {
        let from = Module::new(from).map_err(fault)?;
        resolver
            .resolve(Import {
                from,
                specifier: specifier.into(),
                span: Span::default(),
            })
            .map(|module| module.path().to_string())
            .map_err(diagnostic)
    };
    if script {
        crate::script(entry.path(), read, resolve)
    } else {
        crate::bundle(entry.path(), read, resolve)
    }
}

fn packages(root: &Path) -> Result<Vec<Package>, Fault> {
    let directory = root.join("packages");
    let mut entries = std::fs::read_dir(&directory)
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    entries
        .into_iter()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Fault("package path is not UTF-8".into()))?;
            let module = Module::new(format!("packages/{name}")).map_err(fault)?;
            let manifest =
                std::fs::read_to_string(entry.path().join("package.json")).map_err(error)?;
            Package::parse(module, &manifest).map_err(fault)
        })
        .collect()
}

fn fault(failure: project::Fault) -> Fault {
    Fault(failure.to_string())
}

fn diagnostic(failure: project::Diagnostic) -> Fault {
    Fault(failure.to_string())
}

fn error(failure: std::io::Error) -> Fault {
    Fault(failure.to_string())
}
