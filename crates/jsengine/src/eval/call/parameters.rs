use crate::syntax::Parameters;
use crate::{Addons, Engine, Failure, Value};

impl Engine {
    pub(super) fn parameters(
        &mut self,
        parameters: &Parameters,
        input: (usize, Vec<Value>),
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let (scope, arguments) = input;
        for name in parameters.names() {
            self.reserve(scope, name, true)?;
        }
        let mut arguments = arguments.into_iter();
        for parameter in &parameters.items {
            self.tick()?;
            let mut value = arguments.next().unwrap_or(Value::Undefined);
            if value == Value::Undefined
                && let Some(initializer) = &parameter.initializer
            {
                value = self.named((initializer, Some(&parameter.name)), scope, host)?;
            }
            self.initialize(scope, &parameter.name, value);
        }
        if let Some(name) = &parameters.rest {
            self.tick()?;
            let value = self.array(arguments.map(Some).collect())?;
            self.initialize(scope, name, value);
        }
        Ok(())
    }
}
