use jsengine::{Engine, Fault, Invocation, Value};
use jsruntime::{Addons, Capabilities, Runtime};

struct Host;

impl jsengine::Addons for Host {
    fn call(
        &mut self,
        _: &mut Engine,
        _: usize,
        _: Invocation,
    ) -> Result<Value, jsengine::Failure> {
        Err(Fault("host stopped".into()).into())
    }
}

impl Addons for Host {
    fn install(&mut self, _: &mut Engine, capabilities: &mut Capabilities) -> Result<(), Fault> {
        capabilities.insert("answer".into(), Value::Number(42.0));
        Ok(())
    }
}

#[test]
fn registry() {
    let mut runtime = Runtime::new(Host).unwrap();
    assert_eq!(
        runtime.execute("Symbol[Symbol.plectron].answer;").unwrap(),
        Value::Number(42.0)
    );
    assert_eq!(
        runtime
            .execute("const value = {}; value.plectron = 7; value[Symbol.plectron] === undefined && value.plectron === 7;")
            .unwrap(),
        Value::Boolean(true)
    );
    assert_eq!(
        runtime
            .execute("const descriptor = Reflect.getOwnPropertyDescriptor(Symbol, Symbol.plectron); descriptor.writable === false && descriptor.enumerable === false && descriptor.configurable === false;")
            .unwrap(),
        Value::Boolean(true)
    );
    assert!(runtime.execute("Symbol[Symbol.plectron] = {};").is_err());
}

#[test]
fn source() {
    let mut runtime = Runtime::new(Host).unwrap();
    let source = jsengine::Source::new("(function () { throw 42; });");
    let callback = runtime.source(&source).unwrap();
    let root = runtime.retain(&callback).unwrap();
    runtime.execute("const unrelated = 1;").unwrap();
    runtime.collect().unwrap();
    let failure = runtime
        .call(runtime.held(root).unwrap(), Vec::new())
        .unwrap_err();
    assert_eq!(failure.site().unwrap().source.identity(), source.identity());
    assert_eq!(failure.thrown(), Some(&Value::Number(42.0)));
    runtime.release(root).unwrap();
    let pending = jsengine::Source::new("(async function () { await 1; throw 7; })();");
    runtime.source(&pending).unwrap();
    runtime.checkpoint().unwrap();
    let failures = runtime.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].site().unwrap().source.identity(),
        pending.identity()
    );
    assert_eq!(failures[0].thrown(), Some(&Value::Number(7.0)));
}

#[test]
fn checkpoint() {
    let mut runtime = Runtime::new(Host).unwrap();
    runtime
        .host(|engine, _| {
            let stop = engine.native(0);
            engine.register("stop", stop)
        })
        .unwrap();
    let source = jsengine::Source::new("Promise.resolve(1).then(function () { stop(); });");
    runtime.source(&source).unwrap();
    let failure = runtime.checkpoint().unwrap_err();
    assert_eq!(failure.site().unwrap().source.identity(), source.identity());
    assert_eq!(
        failure.cause(),
        &jsengine::Failure::Fault(Fault("host stopped".into()))
    );
    assert!(runtime.rejections().is_empty());
    assert_eq!(runtime.diagnostic(&failure), Fault("host stopped".into()));
}
