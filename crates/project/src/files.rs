use crate::{Diagnostic, Kind, Module, Provider, Request, Source, Span};
use std::path::{Path, PathBuf};

pub struct Files {
    root: PathBuf,
}

impl Files {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        Ok(Self {
            root: root.as_ref().canonicalize()?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Provider for Files {
    fn read(&self, request: Request) -> Result<Source, Diagnostic> {
        let path = self.root.join(request.module.path());
        let resolved = path.canonicalize().map_err(|_| absent(&request.module))?;
        if !resolved.starts_with(&self.root) {
            return Err(Diagnostic::new(
                request.module,
                Span::default(),
                "module leaves project boundary",
            ));
        }
        let text = std::fs::read_to_string(resolved).map_err(|_| absent(&request.module))?;
        let kind = Kind::of(&request.module)
            .map_err(|fault| Diagnostic::new(request.module.clone(), Span::default(), fault.0))?;
        Ok(Source {
            module: request.module,
            kind,
            text,
        })
    }
}

fn absent(module: &Module) -> Diagnostic {
    Diagnostic::new(
        module.clone(),
        Span::default(),
        format!("module not found: {module}"),
    )
}
