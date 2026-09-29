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
        unreachable!()
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
