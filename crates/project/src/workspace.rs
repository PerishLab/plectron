use crate::{Adapter, Diagnostic, Fault, Files, Linker, Module, Output, Package, Resolver, Target};
use std::path::{Path, PathBuf};

pub struct Workspace<A> {
    root: PathBuf,
    entry: Module,
    linker: Linker<Files, Resolver, A>,
}

impl<A: Adapter> Workspace<A> {
    pub fn open(entry: impl AsRef<Path>, adapter: A) -> Result<Self, Fault> {
        let entry = entry.as_ref().canonicalize().map_err(error)?;
        let root = entry
            .ancestors()
            .find(|path| path.join("pnpm-workspace.yaml").is_file())
            .ok_or_else(|| Fault("project entry is outside a pnpm workspace".into()))?;
        let relative = entry
            .strip_prefix(root)
            .map_err(|failure| Fault(failure.to_string()))?
            .to_str()
            .ok_or_else(|| Fault("project entry path is not UTF-8".into()))?;
        let module = Module::new(relative)?;
        let files = Files::new(root).map_err(error)?;
        let resolver = Resolver::new(packages(root)?)?;
        Ok(Self {
            root: root.into(),
            entry: module,
            linker: Linker::new(files, resolver, adapter),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entry(&self) -> &Module {
        &self.entry
    }

    pub fn link(&mut self, target: Target) -> Result<Output, Diagnostic> {
        self.linker.link(self.entry.clone(), target)
    }
}

fn packages(root: &Path) -> Result<Vec<Package>, Fault> {
    let directory = root.join("packages");
    let mut entries = std::fs::read_dir(directory)
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    entries
        .into_iter()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| package(&entry))
        .collect()
}

fn package(entry: &std::fs::DirEntry) -> Result<Package, Fault> {
    let name = entry
        .file_name()
        .into_string()
        .map_err(|_| Fault("package path is not UTF-8".into()))?;
    let module = Module::new(format!("packages/{name}"))?;
    let manifest = std::fs::read_to_string(entry.path().join("package.json")).map_err(error)?;
    Package::parse(module, &manifest)
}

fn error(failure: std::io::Error) -> Fault {
    Fault(failure.to_string())
}
