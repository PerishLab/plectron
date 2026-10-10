use crate::{Fault, Module};
use std::path::{Component, Path};

impl Module {
    pub fn file(path: impl AsRef<Path>) -> Result<Self, Fault> {
        let parts = path
            .as_ref()
            .components()
            .map(text)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(parts.join("/"))
    }
}

fn text(component: Component<'_>) -> Result<&str, Fault> {
    match component {
        Component::Normal(value) => value
            .to_str()
            .ok_or_else(|| Fault("project entry path is not UTF-8".into())),
        _ => Err(Fault(
            "project file path must be relative and normal".into(),
        )),
    }
}
