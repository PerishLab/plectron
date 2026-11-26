use domengine::{Document, Entry, Kind, Sheet, html};
use plectron::contract::Fault;
use shell::Bundle;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use webengine::resource::{Url, address};

pub fn run(root: &Path, origin: &str, entry: &str, output: &Path) -> Result<(), Fault> {
    let bundle = compile(root, origin, entry)?;
    let bytes = bundle.encode()?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(error)?;
    }
    std::fs::write(output, bytes).map_err(error)?;
    Ok(())
}

pub fn compile(root: &Path, origin: &str, entry: &str) -> Result<Bundle, Fault> {
    let root = root.canonicalize().map_err(error)?;
    if !root.is_dir() {
        return Err(Fault("bundle root must be a directory".into()));
    }
    let normalized = entry.trim_start_matches('/');
    let base = address(origin)?;
    let start = base.join(normalized).map_err(error)?;
    let mut compiler = Compiler {
        root,
        bundle: Bundle::new(origin, start.as_str())?,
        visited: BTreeSet::new(),
    };
    compiler.build(&start)?;
    Ok(compiler.bundle)
}

struct Compiler {
    root: PathBuf,
    bundle: Bundle,
    visited: BTreeSet<String>,
}

impl Compiler {
    fn build(&mut self, start: &Url) -> Result<(), Fault> {
        let file = self.resolve(start.path())?;
        let source = std::fs::read_to_string(&file).map_err(error)?;
        self.bundle
            .insert(start.path(), "text/html", source.as_bytes().to_vec())?;
        self.visited.insert(start.path().to_owned());
        let document = html::parse(&source).map_err(error)?;
        for (reference, kind) in references(&document)? {
            let target = start.join(&reference).map_err(error)?;
            self.asset(&target, kind)?;
        }
        self.siblings(&file)
    }

    fn asset(&mut self, target: &Url, kind: &str) -> Result<(), Fault> {
        let path = target.path().to_owned();
        if !self.visited.insert(path.clone()) {
            return Ok(());
        }
        let asset = self.resolve(&path)?;
        match kind {
            "text/css" => self.stylesheet(target, &asset),
            "text/javascript" => {
                self.bundle
                    .insert(&path, "text/javascript", link(&asset)?.into_bytes())
            }
            mime => {
                let bytes = std::fs::read(&asset).map_err(error)?;
                self.bundle.insert(&path, mime, bytes)
            }
        }
    }

    fn stylesheet(&mut self, target: &Url, file: &Path) -> Result<(), Fault> {
        let source = std::fs::read_to_string(file).map_err(error)?;
        self.bundle
            .insert(target.path(), "text/css", source.as_bytes().to_vec())?;
        let sheet = Sheet::parse(&source).map_err(error)?;
        for font in sheet.fonts() {
            let mut url = target.join(&font.source).map_err(error)?;
            url.set_fragment(None);
            let path = url.path().to_owned();
            if !self.visited.insert(path.clone()) {
                continue;
            }
            let asset = self.resolve(&path)?;
            let bytes = std::fs::read(&asset).map_err(error)?;
            self.bundle.insert(&path, "font/ttf", bytes)?;
        }
        Ok(())
    }

    fn siblings(&mut self, entry: &Path) -> Result<(), Fault> {
        let Some(parent) = entry.parent() else {
            return Ok(());
        };
        let mut paths: Vec<PathBuf> = std::fs::read_dir(parent)
            .map_err(error)?
            .filter_map(|item| item.ok().map(|found| found.path()))
            .filter(|path| path.is_file())
            .collect();
        paths.sort();
        for path in paths {
            let Some(kind) = mime(&path) else {
                continue;
            };
            let canonical = path.canonicalize().map_err(error)?;
            let Ok(relative) = canonical.strip_prefix(&self.root) else {
                continue;
            };
            let key = format!("/{}", slash(relative)?);
            if !self.visited.insert(key.clone()) {
                continue;
            }
            let bytes = std::fs::read(&canonical).map_err(error)?;
            self.bundle.insert(&key, kind, bytes)?;
        }
        Ok(())
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, Fault> {
        let relative = path
            .strip_prefix('/')
            .ok_or_else(|| Fault(format!("expected absolute bundle path: {path}")))?;
        if relative.contains(['\\', '\0', '?', '#']) {
            return Err(Fault(format!("forbidden bundle path: {path}")));
        }
        if Path::new(relative)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(Fault(format!("invalid bundle path components: {path}")));
        }
        let target = self.root.join(relative).canonicalize().map_err(error)?;
        if !target.starts_with(&self.root) || !target.is_file() {
            return Err(Fault(format!("bundle path escapes root: {path}")));
        }
        Ok(target)
    }
}

fn mime(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|part| part.to_str()) {
        Some("json") => Some("application/json"),
        Some("png") => Some("image/png"),
        _ => None,
    }
}

fn link(file: &Path) -> Result<String, Fault> {
    let mut workspace = project::Workspace::open(
        file,
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .map_err(|failure| Fault(failure.0))?;
    let output = workspace
        .link(project::Target::Script)
        .map_err(|failure| Fault(failure.to_string()))?;
    Ok(output.javascript)
}

fn references(document: &Document) -> Result<Vec<(String, &'static str)>, Fault> {
    let mut items = Vec::new();
    for node in document.walk() {
        let entry = document.entry(node).map_err(error)?;
        if let Some(found) = reference(entry) {
            items.push(found);
        }
    }
    Ok(items)
}

fn reference(entry: &Entry) -> Option<(String, &'static str)> {
    let Kind::Element(tag) = &entry.kind else {
        return None;
    };
    let (name, kind) = match tag.as_str() {
        "link" => ("href", "text/css"),
        "script" => ("src", "text/javascript"),
        "img" => ("src", "image/png"),
        _ => return None,
    };
    let value = entry.attributes.get(name).filter(|text| !text.is_empty())?;
    Some((value.clone(), kind))
}

fn slash(path: &Path) -> Result<String, Fault> {
    let mut parts = Vec::new();
    for part in path.components() {
        let Component::Normal(segment) = part else {
            return Err(Fault("invalid relative path".into()));
        };
        parts.push(
            segment
                .to_str()
                .ok_or_else(|| Fault("non-UTF-8 path in bundle".into()))?,
        );
    }
    Ok(parts.join("/"))
}

fn error(failure: impl std::fmt::Display) -> Fault {
    Fault(failure.to_string())
}
