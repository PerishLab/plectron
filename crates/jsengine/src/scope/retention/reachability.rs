use crate::{
    Addons, Engine, Fault, Handle, Value,
    value::{Call, Construct, Content},
};
use std::collections::HashSet;

#[derive(Default)]
pub struct Reachability {
    pub(super) objects: HashSet<Handle>,
    pub(super) scopes: HashSet<usize>,
    pub(super) closures: HashSet<usize>,
    pub(super) bindings: HashSet<usize>,
    pub(super) resolvers: HashSet<usize>,
    pub(super) natives: HashSet<usize>,
    pub(super) activations: usize,
    pub(super) rounds: usize,
}

impl Reachability {
    pub fn contains(&self, value: &Value) -> bool {
        match value {
            Value::Object(handle) => self.objects.contains(handle),
            _ => true,
        }
    }

    pub fn objects(&self) -> usize {
        self.objects.len()
    }
}

struct Walker<'a, H> {
    engine: &'a Engine,
    host: &'a H,
    state: Reachability,
    values: Vec<Value>,
    marked: Vec<Handle>,
    scopes: Vec<usize>,
    bindings: Vec<usize>,
}

impl Engine {
    pub fn reachability(&self, host: &impl Addons) -> Result<Reachability, Fault> {
        if self.turns != 0 || !self.promises.ready() {
            return Err(Fault("reachability requires a quiescent engine".into()));
        }
        let mut walker = Walker {
            engine: self,
            host,
            state: Reachability::default(),
            values: self.retained.values().collect(),
            marked: Vec::new(),
            scopes: vec![0],
            bindings: Vec::new(),
        };
        walker.values.extend([
            Value::Object(self.roots.object),
            Value::Object(self.roots.function),
            Value::Object(self.roots.array),
            Value::Object(self.roots.iterator),
            self.textiter.clone(),
        ]);
        walker.values.extend(self.errors.values().cloned());
        walker.values.extend(self.strings.values().cloned());
        host.roots(&mut |value| walker.values.push(value.clone()));
        self.promises.roots(
            &mut |value| walker.values.push(value.clone()),
            &mut |scope| walker.scopes.push(scope),
            &mut |binding| walker.bindings.push(binding),
        );
        walker.run()
    }
}

impl<H: Addons> Walker<'_, H> {
    fn run(mut self) -> Result<Reachability, Fault> {
        let mut activation = self.engine.weak.activation();
        loop {
            self.drain()?;
            self.state.rounds += 1;
            let marked = std::mem::take(&mut self.marked);
            if !activation.advance(&marked, &mut self.values) {
                break;
            }
        }
        self.state.activations = activation.activated();
        Ok(self.state)
    }

    fn drain(&mut self) -> Result<(), Fault> {
        while !self.values.is_empty() || !self.scopes.is_empty() || !self.bindings.is_empty() {
            if let Some(scope) = self.scopes.pop() {
                self.scope(scope);
                continue;
            }
            if let Some(binding) = self.bindings.pop() {
                if self.state.bindings.insert(binding) {
                    self.values
                        .extend(self.engine.bindings[binding].value.iter().cloned());
                }
                continue;
            }
            if let Value::Object(handle) = self.values.pop().expect("pending value") {
                self.object(handle)?;
            }
        }
        Ok(())
    }

    fn scope(&mut self, scope: usize) {
        if !self.state.scopes.insert(scope) {
            return;
        }
        let frame = &self.engine.scopes[scope];
        self.scopes.extend(frame.parent);
        if let Some(frame) = &frame.frame {
            self.values.push(frame.target.clone());
            self.values.extend(frame.receiver.iter().cloned());
            self.values.extend(frame.home.iter().cloned());
            self.values.extend(frame.constructor.iter().cloned());
        }
        for (_, address) in &frame.bindings {
            self.bindings.push(address.0);
        }
    }

    fn object(&mut self, handle: Handle) -> Result<(), Fault> {
        self.engine.validate(&Value::Object(handle))?;
        if !self.state.objects.insert(handle) {
            return Ok(());
        }
        self.marked.push(handle);
        let object = &self.engine.objects[handle.slot];
        self.values.extend(object.prototype.map(Value::Object));
        for property in object.edges() {
            match &property.content {
                Content::Data { value, .. } => self.values.push(value.clone()),
                Content::Accessor { get, set } => {
                    self.values.extend(get.iter().cloned());
                    self.values.extend(set.iter().cloned());
                }
            }
        }
        if let Some(proxy) = &object.proxy {
            self.values.push(proxy.target.clone());
            self.values.extend(proxy.handler.iter().cloned());
        }
        if let Some(crate::value::access::State::Array(array, _)) = &object.iterator {
            self.values.push(array.clone());
        }
        for initializer in &object.initializers {
            self.values.push(initializer.home.clone());
            self.scopes.push(initializer.scope);
        }
        self.call(object.call);
        self.construct(object.construct);
        self.engine.promises.entry(
            handle.slot,
            &mut |value| self.values.push(value.clone()),
            &mut |scope| self.scopes.push(scope),
            &mut |binding| self.bindings.push(binding),
        );
        self.host
            .edges(handle, &mut |value| self.values.push(value.clone()));
        Ok(())
    }

    fn call(&mut self, call: Option<Call>) {
        match call {
            Some(Call::Closure(slot) | Call::Arrow(slot)) => self.closure(slot),
            Some(Call::Revoke(handle)) => self.values.push(Value::Object(handle)),
            Some(Call::Resolve(id, _)) => {
                self.state.resolvers.insert(id);
                self.engine
                    .promises
                    .resolver(id, &mut |value| self.values.push(value.clone()));
            }
            Some(Call::Native(binding)) => {
                self.state.natives.insert(binding);
                self.host
                    .native(binding, &mut |value| self.values.push(value.clone()));
            }
            _ => {}
        }
    }

    fn construct(&mut self, construct: Option<Construct>) {
        if let Some(Construct::Closure(slot) | Construct::Derived(Some(slot))) = construct {
            self.closure(slot);
        }
    }

    fn closure(&mut self, slot: usize) {
        if !self.state.closures.insert(slot) {
            return;
        }
        let closure = &self.engine.closures[slot];
        self.scopes.push(closure.scope);
        self.values.extend(closure.home.iter().cloned());
    }
}
