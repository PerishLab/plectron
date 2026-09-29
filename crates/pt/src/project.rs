use crate::{Artifact, Fault};
use project::{Files, Linker, Module, Package, Resolver, Target};
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
    let target = if script {
        Target::Script
    } else {
        Target::Module
    };
    let output = Linker::new(files, resolver, crate::Templates)
        .link(entry, target)
        .map_err(diagnostic)?;
    jsengine::Engine::check(&output.javascript).map_err(|failure| Fault(failure.to_string()))?;
    let entry = output.graph.binding(&output.graph.entry);
    Ok(Artifact {
        javascript: output.javascript,
        entry: Some(entry),
    })
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
