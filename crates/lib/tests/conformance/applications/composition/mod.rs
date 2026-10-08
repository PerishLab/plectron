use jsengine::{Addons, Engine, Failure, Invocation, Value};
use project::{Adapters, Kind, Module, Target, Workspace};

mod resources;
mod source;

struct Bare;

impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        panic!("boundary composition must not call native addons")
    }
}

fn check(engine: &mut Engine, module: &str, condition: &str) {
    let code = condition.replace("entry", module);
    assert_eq!(
        engine.execute(&code, &mut Bare).unwrap(),
        Value::Boolean(true),
        "{condition}"
    );
}

#[test]
fn boundary() {
    let root = fixtures::root().parent().unwrap().to_owned();
    let mut workspace = Workspace::open(
        root.join("packages/composition/tests/runtime.ts"),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .unwrap();
    let linked = workspace.link(Target::Module).unwrap();
    let module = linked.graph.binding(&linked.graph.entry);
    assert_eq!(linked.graph.modules[&linked.graph.entry].kind, Kind::Ts);
    let boundary = Module::new("packages/composition/src/boundary.ts").unwrap();
    assert_eq!(linked.graph.modules[&boundary].kind, Kind::Ts);
    assert!(
        linked
            .origins
            .iter()
            .any(|origin| origin.module == boundary)
    );
    let mut engine = Engine::default();
    engine
        .execute(&linked.javascript, &mut Bare)
        .unwrap_or_else(|error| panic!("{}", engine.diagnostic(&error)));
    check(
        &mut engine,
        &module,
        "entry.host.children[0].text === 'content 1';",
    );
    engine
        .execute(&format!("{module}.state.broken = true;"), &mut Bare)
        .unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    check(
        &mut engine,
        &module,
        "entry.region.failed && entry.host.children.length === 1 && entry.host.children[0].text === 'broken content' && entry.retired[0] === 'content 1' && entry.reports[0].phase === 'content';",
    );
    engine
        .execute(
            &format!("{module}.state.broken = false; {module}.region.reset();"),
            &mut Bare,
        )
        .unwrap();
    check(
        &mut engine,
        &module,
        "!entry.region.failed && entry.region.generation === 1 && entry.host.children[0].text === 'content 2' && entry.retired[1] === 'fallback';",
    );
    check(
        &mut engine,
        &module,
        "entry.broken.failed && entry.terminal.children.length === 1 && entry.terminal.children[0].text === 'Content failed' && entry.terminal.children[0].attributes.role === 'alert' && entry.failures[0].primary.message === 'primary' && entry.failures[0].secondary.message === 'secondary' && entry.failures[0].phase === 'failed';",
    );
    check(
        &mut engine,
        &module,
        "entry.missing.failed && entry.inert.children[0].text === 'Content failed';",
    );
    engine
        .execute(&format!("{module}.region.dispose(); {module}.region.dispose(); {module}.broken.dispose(); {module}.missing.dispose(); {module}.scope.dispose();"), &mut Bare)
        .unwrap();
    check(
        &mut engine,
        &module,
        "entry.retired.length === 3 && entry.retired[2] === 'content 2' && !entry.region.reset();",
    );
    assert!(engine.jobs().rejected.is_empty());
}
