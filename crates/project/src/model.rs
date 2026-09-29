use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Module(String);

impl Module {
    pub fn new(path: impl Into<String>) -> Result<Self, Fault> {
        let path = path.into();
        let shape = path.is_empty() || path.starts_with('/') || path.contains('\\');
        let segments = path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
        if shape || segments {
            return Err(Fault(format!("invalid project module: {path}")));
        }
        Ok(Self(path))
    }

    pub fn path(&self) -> &str {
        &self.0
    }

    pub fn kind(&self) -> Result<Kind, Fault> {
        Kind::of(self)
    }

    pub fn binding(&self) -> String {
        let mut binding = String::from("module_");
        for byte in self.path().bytes() {
            use std::fmt::Write;
            write!(binding, "{byte:02x}").expect("writing to a string cannot fail");
        }
        binding
    }
}

impl std::fmt::Display for Module {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Js,
    Ts,
    Pt,
}

impl Kind {
    pub(crate) fn of(module: &Module) -> Result<Self, Fault> {
        match module
            .path()
            .rsplit_once('.')
            .map(|(_, extension)| extension)
        {
            Some("js") => Ok(Self::Js),
            Some("ts") => Ok(Self::Ts),
            Some("pt") => Ok(Self::Pt),
            _ => Err(Fault(format!("unsupported source kind: {module}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Result<Self, Fault> {
        if start > end {
            return Err(Fault("source span starts after its end".into()));
        }
        Ok(Self { start, end })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub module: Module,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn new(module: Module, span: Span, message: impl Into<String>) -> Self {
        Self {
            module,
            span,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{}..{}: {}",
            self.module, self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub String);

impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for Fault {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub module: Module,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub from: Module,
    pub specifier: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub module: Module,
    pub kind: Kind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub module: Module,
    pub kind: Kind,
    pub text: String,
    pub origins: Vec<Origin>,
}

impl Artifact {
    pub fn source(module: Module, kind: Kind, text: String) -> Self {
        let end = text.len();
        Self {
            module: module.clone(),
            kind,
            text,
            origins: vec![Origin {
                generated: Span { start: 0, end },
                module,
                source: Span { start: 0, end },
            }],
        }
    }

    pub fn origin(&self, span: Span) -> Origin {
        self.origins
            .iter()
            .find(|origin| origin.generated.start <= span.start && span.end <= origin.generated.end)
            .map(|origin| origin.project(span))
            .unwrap_or_else(|| Origin {
                generated: span,
                module: self.module.clone(),
                source: Span::default(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub generated: Span,
    pub module: Module,
    pub source: Span,
}

impl Origin {
    pub(crate) fn project(&self, generated: Span) -> Self {
        let exact =
            self.generated.end - self.generated.start == self.source.end - self.source.start;
        let source = if exact {
            Span {
                start: self.source.start + generated.start - self.generated.start,
                end: self.source.start + generated.end - self.generated.start,
            }
        } else {
            self.source
        };
        Self {
            generated,
            module: self.module.clone(),
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub root: Module,
    pub exports: BTreeMap<String, Module>,
}

impl Package {
    pub fn parse(root: Module, manifest: &str) -> Result<Self, Fault> {
        let manifest: serde_json::Value =
            serde_json::from_str(manifest).map_err(|error| Fault(error.to_string()))?;
        let name = manifest["name"]
            .as_str()
            .ok_or_else(|| Fault("package name is missing".into()))?
            .to_string();
        let declared = manifest
            .get("exports")
            .ok_or_else(|| Fault(format!("package exports are missing: {name}")))?;
        let mut exports = BTreeMap::new();
        if let Some(value) = declared.as_str() {
            exports.insert(".".into(), target(root.clone(), value)?);
        } else if let Some(entries) = declared.as_object() {
            for (key, value) in entries {
                let value = value
                    .as_str()
                    .ok_or_else(|| Fault(format!("unsupported package export: {name} {key}")))?;
                if key != "." && !key.starts_with("./") {
                    return Err(Fault(format!("invalid package export: {name} {key}")));
                }
                exports.insert(key.clone(), target(root.clone(), value)?);
            }
        } else {
            return Err(Fault(format!("unsupported package exports: {name}")));
        }
        Ok(Self {
            name,
            root,
            exports,
        })
    }
}

fn target(root: Module, target: &str) -> Result<Module, Fault> {
    let target = target
        .strip_prefix("./")
        .ok_or_else(|| Fault(format!("package export is not relative: {target}")))?;
    let module = Module::new(format!("{}/{target}", root.path()))?;
    module.kind()?;
    Ok(module)
}
