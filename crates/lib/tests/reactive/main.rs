use jsengine::{Addons, Engine, Failure, Invocation, Value};

#[path = "boundary.rs"]
mod boundary;

#[path = "source.rs"]
mod source;

struct Bare;

impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        panic!("reactive core must not need native addons")
    }
}

fn run(engine: &mut Engine, module: &str, source: &str) -> Value {
    engine
        .execute(&source.replace("entry", module), &mut Bare)
        .unwrap()
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("../../packages/reactive/tests")
        .join(name)
}

struct Linked {
    javascript: String,
    entry: String,
}

fn workspace(entry: &std::path::Path) -> Linked {
    let mut workspace = project::Workspace::open(
        entry,
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .unwrap();
    let output = workspace.link(project::Target::Module).unwrap();
    let entry = output.graph.binding(&output.graph.entry);
    Linked {
        javascript: output.javascript,
        entry,
    }
}

#[test]
fn package() {
    let path = fixture("engine.js");
    let artifact = workspace(&path);
    let module = artifact.entry.clone();
    let mut engine = Engine::default();
    engine
        .execute(&artifact.javascript, &mut Bare)
        .unwrap_or_else(|error| panic!("{}", engine.diagnostic(&error)));
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.identity && entry.values.length === 1 && entry.branches.length === 1;"
        ),
        Value::Boolean(true)
    );
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.values[1] === 5 && entry.values.length === 2 && entry.branches[1] === 5;"
        ),
        Value::Boolean(true)
    );
    run(
        &mut engine,
        &module,
        "entry.branch.left = 2; entry.branch.right = 6;",
    );
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.branches.length === 3 && entry.branches[2] === 6;"
        ),
        Value::Boolean(true)
    );
    run(
        &mut engine,
        &module,
        "entry.halt(); entry.branch.right = 7;",
    );
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(&mut engine, &module, "entry.branches.length;"),
        Value::Number(3.0)
    );
    run(&mut engine, &module, "entry.list[2] = 4;");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.traversals.length === 2 && entry.methods.length === 1;"
        ),
        Value::Boolean(true)
    );
    run(&mut engine, &module, "entry.state.count = 5;");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(&mut engine, &module, "entry.parities.length;"),
        Value::Number(1.0)
    );
    run(&mut engine, &module, "entry.state.count = 6;");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.parities.length === 2 && entry.parities[1] === 0;"
        ),
        Value::Boolean(true)
    );
}

#[test]
fn collection() {
    let path = fixture("gc.js");
    let artifact = workspace(&path);
    let module = artifact.entry.clone();
    let mut engine = Engine::default();
    engine.execute(&artifact.javascript, &mut Bare).unwrap();
    let scope = run(&mut engine, &module, "entry.scopeholder.owner;");
    let private = run(&mut engine, &module, "entry.scopeholder.value;");
    assert!(engine.reachability(&Bare).unwrap().contains(&scope));
    assert!(engine.reachability(&Bare).unwrap().contains(&private));
    run(&mut engine, &module, "entry.releasescope();");
    assert!(!engine.reachability(&Bare).unwrap().contains(&scope));
    assert!(!engine.reachability(&Bare).unwrap().contains(&private));
    engine.collect(&mut Bare).unwrap();
    assert!(engine.retain(&scope).is_err());
    assert!(engine.retain(&private).is_err());

    let raw = run(&mut engine, &module, "entry.holder.raw;");
    let proxy = run(&mut engine, &module, "entry.holder.proxy;");
    assert!(engine.reachability(&Bare).unwrap().contains(&raw));
    assert!(engine.reachability(&Bare).unwrap().contains(&proxy));
    run(&mut engine, &module, "entry.release();");
    assert!(!engine.reachability(&Bare).unwrap().contains(&raw));
    assert!(!engine.reachability(&Bare).unwrap().contains(&proxy));
    engine.collect(&mut Bare).unwrap();
    assert!(engine.retain(&raw).is_err());
    assert!(engine.retain(&proxy).is_err());

    let effect = run(&mut engine, &module, "entry.effectholder.value;");
    assert!(engine.reachability(&Bare).unwrap().contains(&effect));
    run(&mut engine, &module, "entry.releaseeffect();");
    assert!(!engine.reachability(&Bare).unwrap().contains(&effect));
    engine.collect(&mut Bare).unwrap();
    assert!(engine.retain(&effect).is_err());

    let pending = run(&mut engine, &module, "entry.pendingholder.value;");
    run(&mut engine, &module, "entry.releasepending();");
    assert!(!engine.reachability(&Bare).unwrap().contains(&pending));
    engine.collect(&mut Bare).unwrap();
    assert!(engine.retain(&pending).is_err());

    let computed = run(&mut engine, &module, "entry.computedholder.value;");
    run(&mut engine, &module, "entry.releasecomputed();");
    assert!(!engine.reachability(&Bare).unwrap().contains(&computed));
    engine.collect(&mut Bare).unwrap();
    assert!(engine.retain(&computed).is_err());

    let boundary = run(&mut engine, &module, "entry.boundaryholder.value;");
    assert!(engine.reachability(&Bare).unwrap().contains(&boundary));
    run(&mut engine, &module, "entry.releaseboundary();");
    engine.checkpoint(&mut Bare).unwrap();
    assert!(!engine.reachability(&Bare).unwrap().contains(&boundary));
    engine.collect(&mut Bare).unwrap();
    assert!(engine.retain(&boundary).is_err());
}

#[test]
fn failures() {
    let path = fixture("errors.js");
    let artifact = workspace(&path);
    let module = artifact.entry.clone();
    let mut engine = Engine::default();
    engine.execute(&artifact.javascript, &mut Bare).unwrap();

    run(&mut engine, &module, "entry.update(2);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 2);
    assert!(
        engine
            .jobs()
            .rejected
            .iter()
            .any(|error| error.contains("first update"))
    );
    assert!(
        engine
            .jobs()
            .rejected
            .iter()
            .any(|error| error.contains("second update"))
    );
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.first.length === 2 && entry.second.length === 2 && entry.siblings.length === 2;"
        ),
        Value::Boolean(true)
    );
    run(&mut engine, &module, "entry.update(3);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.first[2] === 3 && entry.second[2] === 3 && entry.siblings[2] === 3;"
        ),
        Value::Boolean(true)
    );

    run(&mut engine, &module, "entry.derive(2);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 3);
    assert!(
        engine
            .jobs()
            .rejected
            .iter()
            .any(|error| error.contains("computed update"))
    );
    assert_eq!(
        run(&mut engine, &module, "entry.values.length;"),
        Value::Number(1.0)
    );
    run(&mut engine, &module, "entry.derive(3);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.values.length === 2 && entry.values[1] === 6;"
        ),
        Value::Boolean(true)
    );

    run(&mut engine, &module, "entry.recurse(true);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 4);
    assert!(
        engine
            .jobs()
            .rejected
            .iter()
            .any(|error| error.contains("reactive update limit exceeded"))
    );
    run(&mut engine, &module, "entry.recurse(false);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().queued, 0);
}
