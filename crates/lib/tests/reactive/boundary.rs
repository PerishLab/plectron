use super::{Bare, fixture, run};
use jsengine::{Engine, Value};

#[test]
fn propagation() {
    let path = fixture("boundary.js");
    let artifact = super::workspace(&path);
    let module = artifact.entry.clone();
    let mut engine = Engine::default();
    engine.execute(&artifact.javascript, &mut Bare).unwrap();
    assert_eq!(
        run(&mut engine, &module, "entry.nested[0];"),
        Value::Text("handler setup".into())
    );
    run(&mut engine, &module, "entry.update(2);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 0);
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.errors[0] === 'protected update' && entry.inside.length === 2 && entry.sibling.length === 2;"
        ),
        Value::Boolean(true)
    );
    run(&mut engine, &module, "entry.update(3);");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            &mut engine,
            &module,
            "entry.inside.length === 2 && entry.sibling[2] === 3;"
        ),
        Value::Boolean(true)
    );
    run(&mut engine, &module, "entry.updatenested();");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 0);
    assert_eq!(
        run(&mut engine, &module, "entry.nested[1];"),
        Value::Text("nested handler update".into())
    );
    run(&mut engine, &module, "entry.breakhandler();");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 1);
    assert!(engine.jobs().rejected[0].contains("handler update"));
    recovery(&mut engine, &module);
    wrappers(&mut engine, &module);
}

fn recovery(engine: &mut Engine, module: &str) {
    assert_eq!(
        run(
            engine,
            module,
            "entry.fallbackfailures[0] === 'fallback primary' && entry.fallbackfailures[1] === 'fallback secondary' && entry.fallbackfailures[2] === 'failed';"
        ),
        Value::Boolean(true)
    );
    run(engine, module, "entry.failrecovery();");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(
        run(
            engine,
            module,
            "entry.recovery.failed && entry.recovery.generation === 0 && entry.recoveries[0] === 'content:recoverable update';"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        run(engine, module, "entry.resetrecovery();"),
        Value::Boolean(true)
    );
    assert_eq!(
        run(
            engine,
            module,
            "!entry.recovery.failed && entry.recovery.generation === 1 && entry.recovery.value === 2 && entry.recovervalues.length === 3;"
        ),
        Value::Boolean(true)
    );
}

fn wrappers(engine: &mut Engine, module: &str) {
    assert_eq!(
        run(engine, module, "entry.callsync(4);"),
        Value::Number(8.0)
    );
    run(engine, module, "entry.stopsync();");
    assert_eq!(run(engine, module, "entry.callsync(5);"), Value::Undefined);
    assert_eq!(
        run(engine, module, "entry.wrapped.length;"),
        Value::Number(1.0)
    );
    run(engine, module, "entry.callasync();");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 1);
    assert_eq!(
        run(engine, module, "entry.asyncerrors[0];"),
        Value::Text("wrapped async update".into())
    );
    run(engine, module, "entry.startstale();");
    run(engine, module, "entry.stopstale();");
    run(engine, module, "entry.failstale();");
    engine.checkpoint(&mut Bare).unwrap();
    assert_eq!(engine.jobs().rejected.len(), 2);
    assert!(engine.jobs().rejected[1].contains("stale async update"));
}
