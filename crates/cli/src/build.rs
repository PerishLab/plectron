use plectron::contract::Fault;
use std::path::Path;

pub fn run(entry: &Path, output: &Path) -> Result<(), Fault> {
    let mut workspace = project::Workspace::open(
        entry,
        project::Adapters::new(project::Unavailable, pt::Templates),
    )
    .map_err(|failure| Fault(failure.0))?;
    let artifact = workspace
        .link(project::Target::Module)
        .map_err(|failure| Fault(failure.to_string()))?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(error)?;
    }
    std::fs::write(output, artifact.javascript).map_err(error)?;
    Ok(())
}

fn error(failure: std::io::Error) -> Fault {
    Fault(failure.to_string())
}
