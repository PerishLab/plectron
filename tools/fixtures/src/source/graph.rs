use project::{Adapters, Kind, Module, Output, Target, Workspace};
use std::path::Path;

pub(super) fn link(root: &Path, path: &str) -> Result<Output, String> {
    let mut workspace = Workspace::open(
        root.join(path),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .map_err(error)?;
    let output = workspace.link(Target::Script).map_err(error)?;
    let entry = Module::new(path).map_err(error)?;
    if output.graph.entry != entry {
        return Err(format!(
            "linked entry disagrees with original source: {path}"
        ));
    }
    for (module, node) in &output.graph.modules {
        if node.kind != module.kind().map_err(error)? {
            return Err(format!(
                "linked kind disagrees with original source: {module}"
            ));
        }
        let driver = module == &entry && super::role(module.path()) == Ok(super::Role::Driver);
        if node.kind == Kind::Js && !driver {
            return Err(format!("noncanonical graph dependency: {module}"));
        }
        let source = std::fs::read_to_string(root.join(module.path())).map_err(error)?;
        if !source.is_empty() && !output.origins.iter().any(|origin| &origin.module == module) {
            return Err(format!("missing original-source mapping: {module}"));
        }
        if node.kind != Kind::Js && root.join(module.path()).with_extension("js").exists() {
            return Err(format!(
                "generated JS copy beside canonical source: {module}"
            ));
        }
    }
    for origin in &output.origins {
        let source = std::fs::read_to_string(root.join(origin.module.path())).map_err(error)?;
        if origin.source.start > origin.source.end
            || origin.source.end > source.len()
            || !source.is_char_boundary(origin.source.start)
            || !source.is_char_boundary(origin.source.end)
            || origin.generated.start > origin.generated.end
            || origin.generated.end > output.javascript.len()
        {
            return Err(format!(
                "original-source mapping outside source/artifact: {}",
                origin.module
            ));
        }
    }
    Ok(output)
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
