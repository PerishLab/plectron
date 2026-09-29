use crate::{Addons, Engine, Failure, Value, value::property::Descriptor};

impl Engine {
    pub(super) fn describing(
        &mut self,
        target: &Value,
        args: &[Value],
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let key = self.reflectkey(args)?;
        let Some(property) = self.observing(target, &key, host)? else {
            return Ok(Value::Undefined);
        };
        Descriptor::from(property).object(self)
    }

    pub(super) fn defining(
        &mut self,
        target: &Value,
        args: &[Value],
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let key = self.reflectkey(args)?;
        let value = args.get(2).cloned().unwrap_or(Value::Undefined);
        let descriptor = self.parsing(&value, host)?;
        Ok(Value::Boolean(
            self.definingown(target, &key, descriptor, host)?,
        ))
    }
}
