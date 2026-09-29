use crate::{Engine, Error, Failure, Fault, Value};

#[derive(Clone)]
pub(crate) struct Frame {
    pub receiver: Option<Value>,
    pub target: Value,
    pub home: Option<Value>,
    pub constructor: Option<Value>,
}

impl Frame {
    pub fn direct(receiver: Value, home: Option<Value>) -> Self {
        Self {
            receiver: Some(receiver),
            target: Value::Undefined,
            home,
            constructor: None,
        }
    }
}

impl Engine {
    pub(crate) fn context(&mut self, parent: usize, frame: Frame) -> usize {
        let scope = self.scope(parent);
        self.scopes[scope].frame = Some(frame);
        scope
    }

    fn active(&self, mut scope: usize) -> Option<usize> {
        loop {
            let context = &self.scopes[scope];
            if context.frame.is_some() {
                return Some(scope);
            }
            scope = context.parent?;
        }
    }

    pub(crate) fn frame(&self, scope: usize) -> Option<&Frame> {
        self.scopes[self.active(scope)?].frame.as_ref()
    }

    pub(crate) fn receiver(&mut self, scope: usize) -> Result<Value, Failure> {
        match self.frame(scope) {
            Some(frame) => frame
                .receiver
                .clone()
                .ok_or_else(|| self.raise(Error::Reference, "this is not initialized")),
            None => Ok(Value::Undefined),
        }
    }

    pub(crate) fn bind(&mut self, scope: usize, value: Value) -> Result<(), Failure> {
        self.validate(&value)?;
        let scope = self
            .active(scope)
            .ok_or_else(|| Fault("missing constructor context".into()))?;
        let frame = self.scopes[scope].frame.as_mut().expect("activation frame");
        if frame.receiver.is_some() {
            return Err(self.raise(Error::Reference, "this is already initialized"));
        }
        frame.receiver = Some(value);
        Ok(())
    }
}
