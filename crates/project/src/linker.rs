use crate::{
    Adapter, Artifact, Cache, Diagnostic, Edge, Graph, Identity, Import, Module, Node, Origin,
    Output, Provider, Request, Resolve, Source, Span, Syntax, Target, parse,
};
use std::collections::BTreeMap;
use std::fmt::Write;

pub struct Linker<P, R, A> {
    provider: P,
    resolver: R,
    adapter: A,
    cache: BTreeMap<Module, Cached>,
}

#[derive(Clone)]
struct Cached {
    content: Identity,
    kind: crate::Kind,
    artifact: Artifact,
    syntax: Syntax,
}

struct Build {
    active: Vec<Module>,
    modules: BTreeMap<Module, Node>,
    order: Vec<Module>,
    cache: Cache,
}

impl<P, R, A> Linker<P, R, A>
where
    P: Provider,
    R: Resolve,
    A: Adapter,
{
    pub fn new(provider: P, resolver: R, adapter: A) -> Self {
        Self {
            provider,
            resolver,
            adapter,
            cache: BTreeMap::new(),
        }
    }

    pub fn provider(&mut self) -> &mut P {
        &mut self.provider
    }

    pub fn link(&mut self, entry: Module, target: Target) -> Result<Output, Diagnostic> {
        let mut build = Build {
            active: Vec::new(),
            modules: BTreeMap::new(),
            order: Vec::new(),
            cache: Cache::default(),
        };
        self.visit(entry.clone(), None, &mut build)?;
        build.cache.entries = self.cache.len();
        let identity = signature(&entry, &build.modules);
        let graph = Graph {
            identity,
            entry: entry.clone(),
            order: build.order,
            modules: build.modules,
        };
        let (javascript, origins) = self.emit(&graph, target);
        Ok(Output {
            javascript,
            graph,
            cache: build.cache,
            origins,
        })
    }

    fn visit(
        &mut self,
        module: Module,
        incoming: Option<Origin>,
        build: &mut Build,
    ) -> Result<(), Diagnostic> {
        if build.modules.contains_key(&module) {
            return Ok(());
        }
        if let Some(start) = build.active.iter().position(|active| active == &module) {
            return Err(cycle(&module, incoming, &build.active[start..]));
        }
        let cached = self.load(module.clone(), &mut build.cache)?;
        build.active.push(module.clone());
        let mut dependencies = Vec::new();
        for fact in &cached.syntax.dependencies {
            let origin = cached.artifact.origin(fact.source);
            let target = self.resolver.resolve(Import {
                from: origin.module.clone(),
                specifier: fact.specifier.clone(),
                span: origin.source,
            })?;
            dependencies.push(Edge {
                target: target.clone(),
                span: origin.source,
            });
            self.visit(target, Some(origin), build)?;
        }
        build.active.pop();
        build.modules.insert(
            module.clone(),
            Node {
                module: module.clone(),
                kind: cached.kind,
                content: cached.content,
                dependencies,
            },
        );
        build.order.push(module);
        Ok(())
    }

    fn load(&mut self, module: Module, report: &mut Cache) -> Result<Cached, Diagnostic> {
        let source = self.provider.read(Request {
            module: module.clone(),
        })?;
        let content = fingerprint(&source);
        if let Some(cached) = self
            .cache
            .get(&module)
            .filter(|cached| cached.content == content)
        {
            report.hits += 1;
            return Ok(cached.clone());
        }
        report.misses += 1;
        let kind = source.kind;
        let artifact = self.adapter.translate(source)?;
        if artifact.module != module || artifact.kind != crate::Kind::Js {
            return Err(Diagnostic::new(
                module,
                Span::default(),
                "adapter must preserve module identity and produce JavaScript",
            ));
        }
        artifact.validate()?;
        let syntax = parse(&Source {
            module: artifact.module.clone(),
            kind: artifact.kind,
            text: artifact.text.clone(),
        })
        .map_err(|failure| remap(failure, &artifact))?;
        let cached = Cached {
            content,
            kind,
            artifact,
            syntax,
        };
        self.cache.insert(module, cached.clone());
        Ok(cached)
    }

    fn emit(&self, graph: &Graph, target: Target) -> (String, Vec<Origin>) {
        let mut javascript = String::new();
        let mut origins = Vec::new();
        for module in &graph.order {
            let offset = javascript.len();
            let position = origins.len();
            let cached = &self.cache[module];
            let node = &graph.modules[module];
            let imports = imports(cached, node, graph);
            let (body, mapped) = cached.syntax.emit(&cached.artifact);
            let fields = cached
                .syntax
                .exports
                .iter()
                .map(|fact| format!("{}: {}", fact.exported, fact.local))
                .collect::<Vec<_>>()
                .join(", ");
            let name = graph.binding(module);
            let script = target == Target::Script && module == &graph.entry;
            if !script {
                javascript.push_str(&format!("const {name} = (() => {{\n"));
            }
            javascript.push_str(&imports);
            let start = javascript.len();
            javascript.push_str(&body);
            origins.extend(mapped.into_iter().map(|mut origin| {
                origin.generated.start += start;
                origin.generated.end += start;
                origin
            }));
            if script {
                javascript.push_str(&format!("\nconst {name} = {{{fields}}};\n"));
            } else {
                javascript.push_str(&format!("\nreturn {{{fields}}};\n}})();\n"));
            }
            if let [origin] = cached.artifact.origins.as_slice()
                && origin.mapping == crate::Mapping::Range
                && origin.generated
                    == (Span {
                        start: 0,
                        end: cached.artifact.text.len(),
                    })
            {
                let mut projected = origin.clone();
                projected.generated = Span {
                    start: offset,
                    end: javascript.len(),
                };
                origins.truncate(position);
                origins.push(projected);
            }
        }
        (javascript, origins)
    }
}

fn imports(cached: &Cached, node: &Node, graph: &Graph) -> String {
    let mut imports = String::new();
    for (dependency, edge) in node.dependencies.iter().enumerate() {
        for binding in cached
            .syntax
            .bindings
            .iter()
            .filter(|binding| binding.dependency == dependency)
        {
            writeln!(
                imports,
                "const {} = {}.{};",
                binding.local,
                graph.binding(&edge.target),
                binding.imported
            )
            .expect("writing to a string cannot fail");
        }
    }
    imports
}

fn cycle(module: &Module, incoming: Option<Origin>, active: &[Module]) -> Diagnostic {
    let mut path = active.iter().map(Module::path).collect::<Vec<_>>();
    path.push(module.path());
    let message = format!("module cycle: {}", path.join(" -> "));
    match incoming {
        Some(origin) => Diagnostic::new(origin.module, origin.source, message),
        None => Diagnostic::new(module.clone(), Span::default(), message),
    }
}

fn remap(failure: Diagnostic, artifact: &Artifact) -> Diagnostic {
    let origin = artifact.origin(failure.span);
    Diagnostic::new(origin.module, origin.source, failure.message)
}

fn fingerprint(source: &Source) -> Identity {
    let kind = match source.kind {
        crate::Kind::Js => 1,
        crate::Kind::Ts => 2,
        crate::Kind::Pt => 3,
    };
    Identity(hash([&[kind], source.text.as_bytes()]))
}

fn signature(entry: &Module, modules: &BTreeMap<Module, Node>) -> Identity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(entry.path().as_bytes());
    for (module, node) in modules {
        bytes.extend_from_slice(module.path().as_bytes());
        bytes.extend_from_slice(&node.content.0.to_le_bytes());
        for edge in &node.dependencies {
            bytes.extend_from_slice(edge.target.path().as_bytes());
            bytes.extend_from_slice(&edge.span.start.to_le_bytes());
            bytes.extend_from_slice(&edge.span.end.to_le_bytes());
        }
    }
    Identity(hash([bytes.as_slice()]))
}

fn hash<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> u64 {
    parts
        .into_iter()
        .flatten()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}
