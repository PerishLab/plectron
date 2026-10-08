use jsengine::{Atom, Engine, Failure, Fault, Invocation, Property, Root, Value};
use std::collections::BTreeMap;

pub type Capabilities = BTreeMap<String, Value>;

pub trait Addons: jsengine::Addons {
    fn install(
        &mut self,
        engine: &mut Engine,
        capabilities: &mut Capabilities,
    ) -> Result<(), Fault>;
}

pub struct Runtime<A> {
    engine: Engine,
    addons: A,
}

impl<A: Addons> Runtime<A> {
    pub fn new(mut addons: A) -> Result<Self, Fault> {
        let mut engine = Engine::default();
        let mut capabilities = Capabilities::new();
        addons.install(&mut engine, &mut capabilities)?;
        let capabilities = engine.object(capabilities)?;
        engine.freeze(&capabilities)?;
        let constructor = engine.binding("Symbol")?;
        let Value::Symbol(symbol) = engine.get(&constructor, "plectron")? else {
            return Err(Fault("Symbol.plectron is not a Symbol".into()));
        };
        let installed = engine
            .configure(
                &constructor,
                &Atom::Symbol(symbol),
                Property::fixed(capabilities, false),
            )
            .map_err(|failure| engine.diagnostic(&failure))?;
        if !installed {
            return Err(Fault("Plectron capability registry already exists".into()));
        }
        Ok(Self { engine, addons })
    }

    pub fn execute(&mut self, source: &str) -> Result<Value, Failure> {
        self.engine.execute(source, &mut self.addons)
    }

    pub fn source(&mut self, source: &jsengine::Source) -> Result<Value, Failure> {
        self.engine.source(source, &mut self.addons)
    }

    pub fn rejections(&self) -> Vec<Failure> {
        self.engine.rejections()
    }

    pub fn call(&mut self, callback: Value, arguments: Vec<Value>) -> Result<Value, Failure> {
        self.engine.call(callback, arguments, &mut self.addons)
    }

    pub fn invoke(&mut self, callback: Value, invocation: Invocation) -> Result<Value, Failure> {
        self.engine.invoke(callback, invocation, &mut self.addons)
    }

    pub fn construct(
        &mut self,
        constructor: Value,
        arguments: Vec<Value>,
    ) -> Result<Value, Failure> {
        self.engine
            .construct(constructor, arguments, &mut self.addons)
    }

    pub fn read(&mut self, access: &jsengine::Access) -> Result<Value, Failure> {
        self.engine.read(access, &mut self.addons)
    }

    pub fn write(&mut self, access: &jsengine::Access, value: Value) -> Result<(), Failure> {
        self.engine.write(access, value, &mut self.addons)
    }

    pub fn diagnostic(&self, failure: &Failure) -> Fault {
        self.engine.diagnostic(failure)
    }

    pub fn checkpoint(&mut self) -> Result<usize, Failure> {
        self.engine.checkpoint(&mut self.addons)
    }

    pub fn retain(&mut self, value: &Value) -> Result<Root, Fault> {
        self.engine.retain(value)
    }

    pub fn held(&self, root: Root) -> Result<Value, Fault> {
        self.engine.held(root)
    }

    pub fn release(&mut self, root: Root) -> Result<(), Fault> {
        self.engine.release(root)
    }

    pub fn collect(&mut self) -> Result<usize, Fault> {
        self.engine.collect(&mut self.addons)
    }

    pub fn jobs(&self) -> jsengine::Jobs {
        self.engine.jobs()
    }

    pub fn performance(&self) -> jsengine::jit::Report {
        self.engine.performance()
    }

    pub fn activity(&self) -> jsengine::Activity {
        self.engine.activity()
    }

    pub fn collection(&self) -> jsengine::Collection {
        self.engine.collection()
    }

    pub fn slots(&self) -> jsengine::Slots {
        self.engine.slots()
    }

    pub fn tier(&mut self, mode: jsengine::jit::Mode) {
        self.engine.tier(mode);
    }

    pub fn addons(&mut self) -> &mut A {
        &mut self.addons
    }

    pub fn host<T>(
        &mut self,
        operation: impl FnOnce(&mut Engine, &mut A) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        operation(&mut self.engine, &mut self.addons)
    }
}
