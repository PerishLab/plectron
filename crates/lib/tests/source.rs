use jsengine::{Engine, Failure, Fault, Invocation, Value};
use jsruntime::{Capabilities, Runtime, Script};
use project::{Adapters, Linker, Memory, Module, Resolver, Target};

struct Host;
impl jsengine::Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}
impl jsruntime::Addons for Host {
    fn install(&mut self, _: &mut Engine, _: &mut Capabilities) -> Result<(), Fault> {
        Ok(())
    }
}

fn link(sources: &[(&str, &str)]) -> Result<Script, project::Diagnostic> {
    let sources: Vec<_> = sources
        .iter()
        .map(|(name, text)| (Module::new(*name).unwrap(), (*text).into()))
        .collect();
    let entry = sources[0].0.clone();
    Linker::new(
        Memory::new(sources),
        Resolver::new([]).unwrap(),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .link(entry, Target::Script)
    .map(plectron::runtime::source::script)
}

#[test]
fn templates() {
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.execute("let marker = 0;").unwrap();
    for entry in ["entry.ts", "entry.js"] {
        let result = link(&[
            (entry, "import Template from './view.pt'; marker = 1;"),
            ("view.pt", "<template tag=\"div\"><unknown/></template>"),
        ]);
        let failure = result.err().unwrap();
        assert_eq!(failure.module.path(), "view.pt");
        assert_eq!(runtime.execute("marker;").unwrap(), Value::Number(0.0));
    }
    let source = "<template tag=\"div\"><text ref=\"value\"/></template>";
    let script = link(&[
        ("entry.ts", "import Template from './view.pt'; const document = {createElement: (tag) => {throw 'creation';}, createTextNode: (text) => ({text: text})}; Template(document);"),
        ("view.pt", source),
    ]).unwrap();
    let failure = runtime.script(&script).unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "entry.ts");
    assert_eq!(runtime.execute("marker;").unwrap(), Value::Number(0.0));
    let script = link(&[
        (
            "entry.ts",
            "import Template from './view.pt'; Template({});",
        ),
        ("view.pt", source),
    ])
    .unwrap();
    let failure = runtime.script(&script).unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "view.pt");
    assert_eq!(location.span, 0..source.len());
    assert_eq!(location.precision, jsruntime::Precision::Range);
}

#[test]
fn synchronous() {
    let dependency =
        "export const fail = function (value: number): never { const text = '𐐀'; throw value; };";
    let script = link(&[
        ("entry.ts", "import {fail} from './failure.ts'; fail(7);"),
        ("failure.ts", dependency),
    ])
    .unwrap();
    let mut runtime = Runtime::new(Host).unwrap();
    let failure = runtime.script(&script).unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "failure.ts");
    assert_eq!(&dependency[location.span], "throw value;");
    assert_eq!(failure.thrown(), Some(&Value::Number(7.0)));
}

#[test]
fn callbacks() {
    let dependency =
        "export const fail = function (value: number = missing): number { return value; };";
    let script = link(&[
        (
            "entry.ts",
            "import {fail} from './failure.ts'; const saved = fail;",
        ),
        ("failure.ts", dependency),
    ])
    .unwrap();
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.script(&script).unwrap();
    drop(script);
    let callback = runtime.execute("saved;").unwrap();
    let root = runtime.retain(&callback).unwrap();
    runtime.execute("const later = 1;").unwrap();
    runtime.collect().unwrap();
    let failure = runtime
        .call(runtime.held(root).unwrap(), Vec::new())
        .unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "failure.ts");
    assert_eq!(&dependency[location.span], "missing");
    runtime.release(root).unwrap();
}

#[test]
fn asynchronous() {
    let dependency =
        "export const fail = async function (): Promise<never> { await 1; throw 'late'; };";
    let script = link(&[
        (
            "entry.ts",
            "import {fail} from './failure.ts'; fail().then();",
        ),
        ("failure.ts", dependency),
    ])
    .unwrap();
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.script(&script).unwrap();
    drop(script);
    runtime.execute("const later = 1;").unwrap();
    runtime.collect().unwrap();
    runtime.checkpoint().unwrap();
    let failures = runtime.rejections();
    assert_eq!(failures.len(), 1);
    let location = runtime.location(&failures[0]).unwrap();
    assert_eq!(location.resource, "failure.ts");
    assert_eq!(&dependency[location.span], "throw 'late';");
    assert!(runtime.jobs().rejected[0].starts_with("failure.ts:"));
}

#[test]
fn transformed() {
    let text = "const value: number = missing as number;";
    let script = link(&[("entry.ts", text)]).unwrap();
    let mut runtime = Runtime::new(Host).unwrap();
    let failure = runtime.script(&script).unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "entry.ts");
    assert_eq!(&text[location.span], text);
    assert_eq!(location.precision, jsruntime::Precision::Range);
    assert!(runtime.diagnostic(&failure).0.contains(" (range): "));
}

#[test]
fn admission() {
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.execute("let marker = 0;").unwrap();
    let unsupported = link(&[
        ("entry.ts", "import './failure.ts'; marker = 1;"),
        ("failure.ts", "export enum State { Ready }"),
    ]);
    assert_eq!(unsupported.err().unwrap().module.path(), "failure.ts");
    let script = link(&[
        ("entry.ts", "marker = 1; import './failure.js';"),
        ("failure.js", "var rejected = 1;"),
    ])
    .unwrap();
    let failure = runtime.script(&script).unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "failure.js");
    assert_eq!(location.span, 0..3);
    assert_eq!(runtime.execute("marker;").unwrap(), Value::Number(0.0));
}

#[test]
fn anonymous() {
    let script = link(&[(
        "entry.ts",
        "const retained = function (): never { throw 1; };",
    )])
    .unwrap();
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.script(&script).unwrap();
    let failure = runtime
        .execute("try { retained(); } catch (error) { throw error; }")
        .unwrap_err();
    assert!(runtime.location(&failure).is_none());
    assert!(failure.site().is_none());
}
