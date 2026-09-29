use crate::{Artifact, Fault, compile};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

type Names = Vec<(String, String)>;

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
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut body = String::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("import ") {
                let (specifier, names) = import(trimmed)?;
                let dependency = (self.resolve)(path, specifier)?;
                let index = self.visit(&dependency, true)?;
                for (remote, local) in names {
                    imports.push(format!("const {local} = {}.{remote};\n", self.name(index)));
                }
            } else if let Some(rest) = trimmed.strip_prefix("export default function ") {
                let name = rest
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or("");
                if name.is_empty() {
                    return Err(Fault("invalid default function export".into()));
                }
                exports.push(("default".into(), name.into()));
                body.push_str(&line.replacen(
                    &format!("export default function {name}"),
                    &format!("const {name} = function"),
                    1,
                ));
                body.push('\n');
            } else if let Some(rest) = trimmed.strip_prefix("export const ") {
                let name = rest
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or("");
                if name.is_empty() {
                    return Err(Fault("invalid exported const".into()));
                }
                exports.push((name.to_string(), name.to_string()));
                body.push_str(&line.replacen("export const ", "const ", 1));
                body.push('\n');
            } else if let Some(rest) = trimmed.strip_prefix("export {") {
                let entries = rest
                    .strip_suffix("};")
                    .ok_or_else(|| Fault("export list must end on the same line".into()))?;
                for entry in entries.split(',') {
                    let entry = entry.trim();
                    let (local, remote) = entry.split_once(" as ").unwrap_or((entry, entry));
                    exports.push((remote.trim().into(), local.trim().into()));
                }
            } else if trimmed.starts_with("export ") {
                return Err(Fault(format!("unsupported module export: {trimmed}")));
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
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

fn import(source: &str) -> Result<(&str, Names), Fault> {
    let rest = source
        .strip_prefix("import ")
        .expect("caller has import prefix");
    if rest.starts_with('"') {
        return Ok((quoted(rest)?, Vec::new()));
    }
    if let Some((name, specifier)) = rest.split_once(" from ")
        && binding(name)
    {
        return Ok((quoted(specifier)?, vec![("default".into(), name.into())]));
    }
    let (names, specifier) = rest
        .strip_prefix('{')
        .and_then(|value| value.split_once("} from "))
        .ok_or_else(|| Fault(format!("unsupported module import: {source}")))?;
    let names = names
        .split(',')
        .map(|entry| {
            let entry = entry.trim();
            let (remote, local) = entry.split_once(" as ").unwrap_or((entry, entry));
            (remote.trim().into(), local.trim().into())
        })
        .collect();
    Ok((quoted(specifier)?, names))
}

fn binding(name: &str) -> bool {
    !name.starts_with('{')
        && !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn quoted(source: &str) -> Result<&str, Fault> {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix("\";"))
        .ok_or_else(|| Fault(format!("expected quoted module specifier: {source}")))
}
