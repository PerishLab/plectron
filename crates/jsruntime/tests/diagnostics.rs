use jsengine::{Engine, Failure, Fault, Invocation, Value};
use jsruntime::{Capabilities, Diagnostics, Location, Precision, Provenance, Runtime, Script};
use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Host(Diagnostics);
impl jsengine::Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}
impl jsruntime::Addons for Host {
    fn diagnostics(&mut self, diagnostics: Diagnostics) {
        self.0 = diagnostics;
    }
    fn install(&mut self, _: &mut Engine, _: &mut Capabilities) -> Result<(), Fault> {
        Ok(())
    }
}

struct Projection {
    reader: Diagnostics,
    nested: Failure,
    retired: Arc<AtomicUsize>,
}
impl Provenance for Projection {
    fn locate(&self, span: Range<usize>) -> Option<Location> {
        assert!(self.reader.location(&self.nested).is_none());
        Some(Location {
            resource: "entry.ts".into(),
            span,
            precision: Precision::Exact,
        })
    }
}
impl Drop for Projection {
    fn drop(&mut self) {
        assert!(self.reader.location(&self.nested).is_none());
        self.retired.fetch_add(1, Ordering::Relaxed);
    }
}

fn script(runtime: &mut Runtime<Host>, text: &str, retired: &Arc<AtomicUsize>) -> Script {
    let source = jsengine::Source::new("throw 'unregistered';");
    let nested = runtime.source(&source).unwrap_err();
    Script::new(
        text,
        Arc::new(Projection {
            reader: runtime.addons().0.clone(),
            nested,
            retired: retired.clone(),
        }),
    )
}

#[test]
fn ownership() {
    let mut runtime = Runtime::new(Host::default()).unwrap();
    let reader = runtime.addons().0.clone();
    let retired = Arc::new(AtomicUsize::new(0));
    let input = script(&mut runtime, "throw 'mapped';", &retired);
    let failure = runtime.script(&input).unwrap_err();
    assert_eq!(reader.location(&failure).unwrap().resource, "entry.ts");
    let mut other = Runtime::new(Host::default()).unwrap();
    assert!(other.addons().0.location(&failure).is_none());
    drop(input);
    drop(runtime);
    assert!(reader.location(&failure).is_none());
    assert_eq!(retired.load(Ordering::Relaxed), 1);
}

#[test]
fn collection() {
    let mut runtime = Runtime::new(Host::default()).unwrap();
    let reader = runtime.addons().0.clone();
    let retired = Arc::new(AtomicUsize::new(0));
    let input = script(
        &mut runtime,
        "(function () { throw 'retained'; });",
        &retired,
    );
    let weak = input.source().downgrade();
    let value = runtime.script(&input).unwrap();
    let root = runtime.retain(&value).unwrap();
    drop(input);
    runtime.collect().unwrap();
    let failure = runtime
        .call(runtime.held(root).unwrap(), Vec::new())
        .unwrap_err();
    assert_eq!(reader.location(&failure).unwrap().resource, "entry.ts");
    drop(failure);
    runtime.release(root).unwrap();
    runtime.collect().unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(retired.load(Ordering::Relaxed), 1);
}
