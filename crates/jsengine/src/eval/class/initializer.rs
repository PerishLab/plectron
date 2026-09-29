use crate::scope::Frame;
use crate::value::Initializer;
use crate::{Addons, Engine, Failure, Property, Value};

impl Engine {
    pub(crate) fn initializers(&self, constructor: &Value) -> Vec<Initializer> {
        let Value::Object(handle) = constructor else {
            unreachable!()
        };
        self.objects[handle.slot].initializers.clone()
    }

    pub(crate) fn populate(
        &mut self,
        receiver: &Value,
        initializers: &[Initializer],
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        for initializer in initializers {
            self.tick()?;
            let scope = self.context(
                initializer.scope,
                Frame::direct(receiver.clone(), Some(initializer.home.clone())),
            );
            let value = match &initializer.expression {
                Some(expression) => {
                    self.named((expression, initializer.name.string()), scope, host)?
                }
                None => Value::Undefined,
            };
            self.install(receiver, &initializer.name, Property::data(value))?;
        }
        Ok(())
    }
}
