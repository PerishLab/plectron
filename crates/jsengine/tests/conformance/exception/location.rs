use super::super::native::Bare;
use jsengine::{Addons, Engine, Failure, Fault, Invocation, Source, Value};

fn text(failure: &Failure) -> &str {
    let site = failure.site().expect("source location");
    &site.source.text()[site.span.clone()]
}

#[test]
fn synchronous() {
    let mut engine = Engine::default();
    let source = Source::new("const text = '𐐀'; throw 42;");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert_eq!(failure.thrown(), Some(&Value::Number(42.0)));
    assert_eq!(text(&failure), "throw 42;");
    assert_eq!(failure.site().unwrap().source.identity(), source.identity());
    assert_eq!(
        failure.site().unwrap().span.start,
        source.text().find("throw").unwrap()
    );
    assert!(
        engine
            .execute("throw 1;", &mut Bare)
            .unwrap_err()
            .site()
            .is_none()
    );
}

#[test]
fn syntax() {
    let mut engine = Engine::default();
    engine.execute("let value = 1;", &mut Bare).unwrap();
    let source = Source::new("value = 2; var rejected = 0;");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert_eq!(text(&failure), "var");
    assert!(engine.diagnostic(&failure).0.contains("SyntaxError:"));
    assert_eq!(
        engine.execute("value;", &mut Bare).unwrap(),
        Value::Number(1.0)
    );
}

#[test]
fn callback() {
    let mut engine = Engine::default();
    let source = Source::new("(function () { throw 'retained'; });");
    let identity = source.identity();
    let callback = engine.source(&source, &mut Bare).unwrap();
    let root = engine.retain(&callback).unwrap();
    drop(source);
    engine.execute("const unrelated = 1;", &mut Bare).unwrap();
    engine.collect(&mut Bare).unwrap();
    let callback = engine.held(root).unwrap();
    let failure = engine.call(callback, Vec::new(), &mut Bare).unwrap_err();
    assert_eq!(text(&failure), "throw 'retained';");
    assert_eq!(failure.site().unwrap().source.identity(), identity);
    engine.release(root).unwrap();
}

#[test]
fn rethrow() {
    let mut engine = Engine::default();
    let source =
        Source::new("const error = {}; try { throw error; } catch (value) { throw value; }");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert_eq!(text(&failure), "throw value;");
    assert_eq!(failure.thrown(), Some(&engine.binding("error").unwrap()));
    let source = Source::new("try { throw error; } catch {} throw error;");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert_eq!(
        failure.site().unwrap().span.start,
        source.text().rfind("throw").unwrap()
    );
}

#[test]
fn finalization() {
    let mut engine = Engine::default();
    let source = Source::new("try { throw 1; } finally { throw 2; }");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert_eq!(text(&failure), "throw 2;");
    let source = Source::new("try { throw 3; } finally { 4; }");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert_eq!(text(&failure), "throw 3;");
    engine
        .source(&Source::new("try { throw 5; } catch {}"), &mut Bare)
        .unwrap();
    assert!(
        engine
            .call(Value::Null, Vec::new(), &mut Bare)
            .unwrap_err()
            .site()
            .is_none()
    );
}

#[test]
fn asynchronous() {
    let mut engine = Engine::default();
    let source = Source::new(
        "const fail = async function () { await Promise.resolve(1); throw 42; }; fail();",
    );
    let identity = source.identity();
    engine.source(&source, &mut Bare).unwrap();
    drop(source);
    engine.execute("const unrelated = 1;", &mut Bare).unwrap();
    engine.collect(&mut Bare).unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(text(&failures[0]), "throw 42;");
    assert_eq!(failures[0].site().unwrap().source.identity(), identity);
}

#[test]
fn forwarding() {
    let mut engine = Engine::default();
    let source = Source::new("const rejected = Promise.reject('origin');");
    engine.source(&source, &mut Bare).unwrap();
    let next = Source::new(
        "const forwarded = rejected.then().finally(function () { return Promise.resolve(1); });",
    );
    engine.source(&next, &mut Bare).unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].site().unwrap().source.identity(),
        source.identity()
    );
    assert_eq!(text(&failures[0]), source.text());
}

#[test]
fn handled() {
    let mut engine = Engine::default();
    let source = Source::new("const rejected = Promise.reject('old');");
    engine.source(&source, &mut Bare).unwrap();
    let handler = Source::new("rejected.catch(function (value) { throw value; });");
    engine.source(&handler, &mut Bare).unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(text(&failures[0]), "throw value;");
    assert_eq!(
        failures[0].site().unwrap().source.identity(),
        handler.identity()
    );
}

#[test]
fn identity() {
    let first = Source::new("throw 1;");
    let second = Source::new(first.text());
    assert_ne!(first.identity(), second.identity());
    assert_eq!(first.identity(), first.clone().identity());
}

struct Halt;
impl Addons for Halt {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        Err(Fault("host stopped".into()).into())
    }
}

#[test]
fn host() {
    let mut engine = Engine::default();
    let stop = engine.native(0);
    engine.register("stop", stop).unwrap();
    let source = Source::new(
        "let marker = 0; try { stop(); } catch { marker = 1; } finally { marker = 2; }",
    );
    let failure = engine.source(&source, &mut Halt).unwrap_err();
    assert_eq!(
        failure.cause(),
        &Failure::Fault(Fault("host stopped".into()))
    );
    assert_eq!(text(&failure), "stop();");
    assert_eq!(engine.binding("marker").unwrap(), Value::Number(0.0));
}

#[test]
fn budget() {
    let mut engine = Engine::default();
    let source = Source::new("const repeat = function () { repeat(); }; repeat();");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    assert!(matches!(failure.cause(), Failure::Fault(_)));
    assert!(failure.site().is_some());
}

#[test]
fn adoption() {
    let mut engine = Engine::default();
    let source = Source::new(
        "const seed = Promise.reject({}); const initial = new Promise(function (resolve) { resolve(seed); });",
    );
    engine.source(&source, &mut Bare).unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(text(&failures[0]), "const seed = Promise.reject({});");
    engine.collect(&mut Bare).unwrap();
    assert!(engine.rejections()[0].thrown().is_some());
}

#[test]
fn executor() {
    let mut engine = Engine::default();
    let source = Source::new("new Promise(function () { throw 'executor'; });");
    engine.source(&source, &mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(text(&failures[0]), "throw 'executor';");
    engine
        .source(
            &Source::new("new Promise(function (resolve) { resolve(1); throw 'ignored'; });"),
            &mut Bare,
        )
        .unwrap();
    assert_eq!(engine.rejections().len(), 1);
}

#[test]
fn awaiting() {
    let mut engine = Engine::default();
    let source = Source::new("const rejected = Promise.reject('origin');");
    engine.source(&source, &mut Bare).unwrap();
    let consumer = Source::new(
        "(async function () { try { await rejected; } catch (value) { await 1; throw value; } })();",
    );
    engine.source(&consumer, &mut Bare).unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    assert_eq!(text(&failures[0]), "throw value;");
    assert_eq!(
        failures[0].site().unwrap().source.identity(),
        consumer.identity()
    );
}

struct Reentry(Source);
impl Addons for Reentry {
    fn call(&mut self, engine: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        engine.source(&self.0, &mut Bare)?;
        Ok(Value::Undefined)
    }
}

#[test]
fn reentry() {
    let mut engine = Engine::default();
    let nested = engine.native(0);
    engine.register("nested", nested).unwrap();
    let source = Source::new("nested(); throw 'outer';");
    let mut host = Reentry(Source::new("try { throw 'handled'; } catch {}"));
    let failure = engine.source(&source, &mut host).unwrap_err();
    assert_eq!(text(&failure), "throw 'outer';");
    assert_eq!(failure.site().unwrap().source.identity(), source.identity());
    host.0 = Source::new("throw 'inner';");
    let failure = engine.source(&source, &mut host).unwrap_err();
    assert_eq!(text(&failure), "throw 'inner';");
    assert_eq!(failure.site().unwrap().source.identity(), host.0.identity());
}
