use jsengine::{Engine, Failure, Fault, Invocation, Value};
use jsruntime::{Capabilities, Location, Precision, Provenance, Runtime, Script};
use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Host;
impl jsengine::Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        Err(Fault("host stopped".into()).into())
    }
}
impl jsruntime::Addons for Host {
    fn install(&mut self, _: &mut Engine, _: &mut Capabilities) -> Result<(), Fault> {
        Ok(())
    }
}

struct Projection {
    name: String,
    retired: Arc<AtomicUsize>,
}
impl Provenance for Projection {
    fn locate(&self, span: Range<usize>) -> Option<Location> {
        Some(Location {
            resource: self.name.clone(),
            span,
            precision: Precision::Exact,
        })
    }
}
impl Drop for Projection {
    fn drop(&mut self) {
        self.retired.fetch_add(1, Ordering::Relaxed);
    }
}
fn script(text: &str, name: &str, retired: &Arc<AtomicUsize>) -> Script {
    Script::new(
        text,
        Arc::new(Projection {
            name: name.into(),
            retired: retired.clone(),
        }),
    )
}

#[test]
fn synchronous() {
    let mut runtime = Runtime::new(Host).unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let input = script("const text = '𐐀'; throw 7;", "entry.ts", &retired);
    let failure = runtime.script(&input).unwrap_err();
    let site = failure.site().unwrap();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "entry.ts");
    assert_eq!(location.span, site.span);
    assert_eq!(location.precision, Precision::Exact);
    assert_eq!(failure.thrown(), Some(&Value::Number(7.0)));
    assert!(runtime.diagnostic(&failure).0.starts_with("entry.ts:"));
    let anonymous = runtime.execute("throw 8;").unwrap_err();
    assert!(runtime.location(&anonymous).is_none());
    assert_eq!(runtime.diagnostic(&anonymous).0, "uncaught exception: 8");
}

#[test]
fn callbacks() {
    let mut runtime = Runtime::new(Host).unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let input = script(
        "(function () { throw 'retained'; });",
        "definition.ts",
        &retired,
    );
    let callback = runtime.script(&input).unwrap();
    let root = runtime.retain(&callback).unwrap();
    let weak = input.source().downgrade();
    drop(input);
    let later = script("const unrelated = 1;", "later.ts", &retired);
    runtime.script(&later).unwrap();
    runtime.collect().unwrap();
    assert!(weak.upgrade().is_some());
    let failure = runtime
        .call(runtime.held(root).unwrap(), Vec::new())
        .unwrap_err();
    assert_eq!(
        runtime.location(&failure).unwrap().resource,
        "definition.ts"
    );
    drop(failure);
    runtime.release(root).unwrap();
    runtime.collect().unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(retired.load(Ordering::Relaxed), 1);
}

#[test]
fn asynchronous() {
    let mut runtime = Runtime::new(Host).unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let input = script(
        "const rejected = (async function () { await 1; throw 9; })();",
        "async.ts",
        &retired,
    );
    runtime.script(&input).unwrap();
    drop(input);
    let forwarding = script(
        "rejected.then().finally(function () {});",
        "forward.ts",
        &retired,
    );
    runtime.script(&forwarding).unwrap();
    drop(forwarding);
    runtime.collect().unwrap();
    runtime.checkpoint().unwrap();
    let failures = runtime.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(runtime.location(&failures[0]).unwrap().resource, "async.ts");
    assert!(runtime.jobs().rejected[0].starts_with("async.ts:"));
}

#[test]
fn rethrow() {
    let mut runtime = Runtime::new(Host).unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let first = script("const rejected = Promise.reject({});", "first.ts", &retired);
    runtime.script(&first).unwrap();
    let second = script(
        "rejected.catch(function (error) { throw error; });",
        "second.ts",
        &retired,
    );
    runtime.script(&second).unwrap();
    runtime.checkpoint().unwrap();
    let failure = runtime.rejections().pop().unwrap();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "second.ts");
    assert_eq!(&second.source().text()[location.span], "throw error;");
}

#[test]
fn faults() {
    let mut runtime = Runtime::new(Host).unwrap();
    runtime
        .host(|engine, _| {
            let stop = engine.native(0);
            engine.register("stop", stop)
        })
        .unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let input = script(
        "Promise.resolve().then(function () { stop(); });",
        "host.ts",
        &retired,
    );
    runtime.script(&input).unwrap();
    let failure = runtime.checkpoint().unwrap_err();
    assert_eq!(
        failure.cause(),
        &Failure::Fault(Fault("host stopped".into()))
    );
    assert_eq!(runtime.location(&failure).unwrap().resource, "host.ts");
    assert!(runtime.diagnostic(&failure).0.ends_with("host stopped"));
    assert!(runtime.rejections().is_empty());
}

#[test]
fn syntax() {
    let mut runtime = Runtime::new(Host).unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    runtime.execute("let marker = 0;").unwrap();
    let input = script("marker = 1; var rejected;", "syntax.ts", &retired);
    let failure = runtime.script(&input).unwrap_err();
    assert_eq!(runtime.location(&failure).unwrap().span, 12..15);
    assert_eq!(runtime.execute("marker;").unwrap(), Value::Number(0.0));
}

#[test]
fn identity() {
    let text: Arc<str> = Arc::from("throw 1;");
    let first = jsengine::Source::new(text.clone());
    let second = jsengine::Source::new(text.clone());
    let weak = first.downgrade();
    assert_ne!(first.identity(), second.identity());
    drop(first);
    assert!(weak.upgrade().is_none());
    assert_eq!(second.text(), text.as_ref());
}
