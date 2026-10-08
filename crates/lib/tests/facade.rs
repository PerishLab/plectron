use jsengine::{Engine, Failure, Fault, Invocation, Value};
use jsruntime::{Capabilities, Runtime};
use project::{Adapter, Adapters, Kind, Linker, Memory, Module, Resolver, Target};

const NAMES: [&str; 11] = [
    "platform",
    "console",
    "document",
    "events",
    "scroll",
    "visual",
    "bridge",
    "shortcuts",
    "fetch",
    "requests",
    "surface",
];

struct Host(Option<f64>);

impl jsengine::Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}

impl jsruntime::Addons for Host {
    fn install(&mut self, _: &mut Engine, capabilities: &mut Capabilities) -> Result<(), Fault> {
        if let Some(offset) = self.0 {
            for (index, name) in NAMES.into_iter().enumerate() {
                capabilities.insert(name.into(), Value::Number(offset + index as f64));
            }
        }
        Ok(())
    }
}

fn output(path: &str, text: &str) -> project::Output {
    let (package, engine) = fixtures::engine();
    let entry = Module::new(path).unwrap();
    Linker::new(
        Memory::new([(entry.clone(), text.into()), engine]),
        Resolver::new([package]).unwrap(),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .link(entry, Target::Script)
    .unwrap()
}

fn facade(path: &str) -> project::Output {
    let names = NAMES
        .map(|name| format!("{name} as capability_{name}"))
        .join(", ");
    let fields = NAMES
        .map(|name| format!("{name}: capability_{name}"))
        .join(", ");
    output(
        path,
        &format!("import {{{names}}} from '@plectron/engine'; const engine = {{{fields}}};"),
    )
}

#[test]
fn source() {
    let root = fixtures::root().join("../packages/engine");
    assert!(!root.join("src/index.js").exists());
    let mut workspace = project::Workspace::open(
        root.join("src/index.ts"),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .unwrap();
    let actual = workspace.link(Target::Module).unwrap();
    let facade = Module::new("packages/engine/src/index.ts").unwrap();
    assert_eq!(actual.graph.entry, facade);
    assert_eq!(actual.graph.modules[&facade].kind, Kind::Ts);
    let (_, (module, text)) = fixtures::engine();
    let artifact = typescript::Typescript
        .translate(project::Source {
            module,
            kind: Kind::Ts,
            text,
        })
        .unwrap();
    let syntax = project::parse(&project::Source {
        module: artifact.module,
        kind: Kind::Js,
        text: artifact.text,
    })
    .unwrap();
    let names: Vec<_> = syntax
        .exports
        .iter()
        .map(|export| export.exported.as_str())
        .collect();
    assert_eq!(names, NAMES);
    for path in ["fixture.js", "fixture.ts"] {
        let linked = self::facade(path);
        assert_eq!(linked.graph.modules[&facade].kind, Kind::Ts);
        assert_eq!(linked.graph.order[0], facade);
        assert!(linked.origins.iter().any(|origin| origin.module == facade));
        let mut runtime = Runtime::new(Host(Some(1.0))).unwrap();
        runtime
            .script(&plectron::runtime::source::script(linked))
            .unwrap();
        assert_eq!(
            runtime.execute("engine.document;").unwrap(),
            Value::Number(3.0)
        );
    }
}

#[test]
fn registry() {
    let linked = facade("fixture.ts");
    let mut runtime = Runtime::new(Host(Some(1.0))).unwrap();
    runtime
        .script(&plectron::runtime::source::script(linked))
        .unwrap();
    assert_eq!(
        runtime.execute("Reflect.ownKeys(engine).length;").unwrap(),
        Value::Number(NAMES.len() as f64)
    );
    for (index, name) in NAMES.into_iter().enumerate() {
        assert_eq!(
            runtime.execute(&format!("engine.{name};")).unwrap(),
            Value::Number(1.0 + index as f64)
        );
        assert_eq!(
            runtime.execute(&format!("const held_{name} = Reflect.getOwnPropertyDescriptor(Symbol[Symbol.plectron], '{name}'); held_{name}.writable === false && held_{name}.configurable === false && engine.{name} === Symbol[Symbol.plectron].{name};")).unwrap(),
            Value::Boolean(true)
        );
    }
    assert_eq!(
        runtime.execute("!Reflect.isExtensible(Symbol[Symbol.plectron]) && !Reflect.set(Symbol[Symbol.plectron], 'extra', 1) && typeof document === 'undefined' && typeof platform === 'undefined';").unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn ownership() {
    let linked = output(
        "fixture.ts",
        "import {document} from '@plectron/engine'; const saved = () => document;",
    );
    let script = plectron::runtime::source::script(linked);
    let mut first = Runtime::new(Host(Some(1.0))).unwrap();
    let mut second = Runtime::new(Host(Some(101.0))).unwrap();
    first.script(&script).unwrap();
    second.script(&script).unwrap();
    drop(script);
    first.execute("const later = 1;").unwrap();
    second.execute("const later = 2;").unwrap();
    first.collect().unwrap();
    second.collect().unwrap();
    assert_eq!(first.execute("saved();").unwrap(), Value::Number(3.0));
    assert_eq!(second.execute("saved();").unwrap(), Value::Number(103.0));
}

#[test]
fn absence() {
    let linked = facade("fixture.ts");
    let mut runtime = Runtime::new(Host(None)).unwrap();
    runtime
        .script(&plectron::runtime::source::script(linked))
        .unwrap();
    for name in NAMES {
        assert_eq!(
            runtime.execute(&format!("engine.{name};")).unwrap(),
            Value::Undefined
        );
    }
}
