use plectron::contract::Fault;
use serde::Deserialize;
use std::path::{Component, Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    entry: String,
}

pub fn load(root: &Path) -> Result<plectron::runtime::Script, Fault> {
    let manifest = root.join("plectron.toml");
    let source = std::fs::read_to_string(&manifest).map_err(error)?;
    let entry = path(&source)?;
    file(&root.join(entry))
}

pub fn file(path: &Path) -> Result<plectron::runtime::Script, Fault> {
    let mut workspace = project::Workspace::open(
        path,
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .map_err(|failure| Fault(failure.0))?;
    workspace
        .link(project::Target::Script)
        .map(plectron::runtime::source::script)
        .map_err(|failure| Fault(failure.to_string()))
}

pub fn path(source: &str) -> Result<String, Fault> {
    let manifest: Manifest =
        toml::from_str(source).map_err(|failure| Fault(failure.to_string()))?;
    let entry = Path::new(&manifest.entry);
    if manifest.entry.is_empty()
        || !entry
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(Fault(
            "plectron.toml entry must be relative to its app directory".into(),
        ));
    }
    Ok(manifest.entry)
}

fn error(failure: std::io::Error) -> Fault {
    Fault(failure.to_string())
}
