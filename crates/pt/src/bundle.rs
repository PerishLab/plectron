use crate::{Artifact, Fault, compile};
use project::{Kind, Module, Source};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn bundle(
    entry: &str,
    read: impl Fn(&str) -> Result<String, Fault>,
    resolve: impl Fn(&str, &str) -> Result<String, Fault>,
) -> Result<Artifact, Fault> {
    let mut linker = Linker {
        read: &read,
        resolve: &resolve,
        modules: Vec::new(),
        indices: BTreeMap::new(),
        active: BTreeSet::new(),
        prefix: "module".into(),
    };
    linker.visit(entry, true)?;
    finish(linker.modules)
}

pub fn script(
    entry: &str,
    read: impl Fn(&str) -> Result<String, Fault>,
    resolve: impl Fn(&str, &str) -> Result<String, Fault>,
) -> Result<Artifact, Fault> {
    let mut linker = Linker {
        read: &read,
        resolve: &resolve,
        modules: Vec::new(),
        indices: BTreeMap::new(),
        active: BTreeSet::new(),
        prefix: prefix(entry),
    };
    linker.visit(entry, false)?;
    finish(linker.modules)
}

fn finish(modules: Vec<String>) -> Result<Artifact, Fault> {
    let javascript = modules.join("\n");
    jsengine::Engine::check(&javascript).map_err(|failure| Fault(failure.to_string()))?;
    Ok(Artifact { javascript })
}

struct Linker<'a, R, P> {
    read: &'a R,
    resolve: &'a P,
    modules: Vec<String>,
    indices: BTreeMap<String, usize>,
    active: BTreeSet<String>,
    prefix: String,
}

impl<R, P> Linker<'_, R, P>
where
    R: Fn(&str) -> Result<String, Fault>,
    P: Fn(&str, &str) -> Result<String, Fault>,
{
    fn visit(&mut self, path: &str, wrapped: bool) -> Result<usize, Fault> {
        if let Some(index) = self.indices.get(path) {
            return Ok(*index);
        }
        if !self.active.insert(path.into()) {
            return Err(Fault(format!("module cycle at {path}")));
        }
        let source = (self.read)(path)?;
        let source = if path.ends_with(".pt") {
            compile(&source)?.javascript
        } else {
            source
        };
        let identity = Module::new(path).map_err(|failure| Fault(failure.to_string()))?;
        let kind = identity.kind().unwrap_or(if path.ends_with(".pt") {
            Kind::Pt
        } else {
            Kind::Js
        });
        let syntax = project::parse(&Source {
            module: identity,
            kind,
            text: source.clone(),
        })
        .map_err(|failure| Fault(failure.to_string()))?;
        let mut imports = Vec::new();
        for (dependency, fact) in syntax.dependencies.iter().enumerate() {
            let path = (self.resolve)(path, &fact.specifier)?;
            let index = self.visit(&path, true)?;
            for binding in syntax
                .bindings
                .iter()
                .filter(|binding| binding.dependency == dependency)
            {
                imports.push(format!(
                    "const {} = {}.{};\n",
                    binding.local,
                    self.name(index),
                    binding.imported
                ));
            }
        }
        let exports = syntax
            .exports
            .iter()
            .map(|fact| (fact.exported.clone(), fact.local.clone()))
            .collect::<Vec<_>>();
        let body = syntax.render(&source);
        let index = self.modules.len();
        let fields = exports
            .iter()
            .map(|(remote, local)| format!("{remote}: {local}"))
            .collect::<Vec<_>>()
            .join(", ");
        let imports = imports.join("");
        let name = self.name(index);
        let module = if wrapped {
            format!("const {name} = (() => {{\n{imports}{body}\nreturn {{{fields}}};\n}})();\n")
        } else {
            format!("{imports}{body}\nconst {name} = {{{fields}}};\n")
        };
        self.modules.push(module);
        self.indices.insert(path.into(), index);
        self.active.remove(path);
        Ok(index)
    }

    fn name(&self, index: usize) -> String {
        format!("{}{index}", self.prefix)
    }
}

fn prefix(entry: &str) -> String {
    let identity = Path::new(entry)
        .components()
        .rev()
        .take(4)
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let hash = identity.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("module_{hash:x}_")
}
