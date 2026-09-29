use crate::value::{Call, Construct};
use crate::{Engine, Error, Failure, Fault, Property, Value};

impl Engine {
    pub(crate) fn arrays(&mut self) -> Result<(), Fault> {
        let prototype = Value::Object(self.roots.array);
        let constructor = self.constructor(Call::Array, Construct::Array, prototype);
        self.signature(&constructor, "Array", 1);
        let test = self.function(Call::Arrays);
        self.signature(&test, "isArray", 1);
        let Value::Object(handle) = constructor else {
            unreachable!();
        };
        self.objects[handle.slot]
            .fields
            .insert("isArray".into(), Property::hidden(test));
        self.register("Array", Value::Object(handle))
    }

    pub(crate) fn arraycall(&mut self, args: &[Value]) -> Result<Value, Failure> {
        if let [Value::Number(length)] = args {
            let finite = length.is_finite() && *length >= 0.0;
            let bounded = *length <= u32::MAX as f64 && length.fract() == 0.0;
            if !finite || !bounded {
                return Err(self.raise(Error::Range, "invalid array length"));
            }
            if *length > 4096.0 {
                return Err(Fault("array length budget exhausted".into()).into());
            }
            return Ok(self.array(vec![None; *length as usize])?);
        }
        Ok(self.array(args.iter().cloned().map(Some).collect())?)
    }

    pub(crate) fn isarray(&mut self, args: &[Value]) -> Result<Value, Failure> {
        let mut value = args.first().cloned().unwrap_or(Value::Undefined);
        loop {
            let Value::Object(handle) = value else {
                return Ok(Value::Boolean(false));
            };
            if let Some(proxy) = self.proxydata(&Value::Object(handle)) {
                let Some(_) = proxy.handler else {
                    return Err(self.raise(Error::Type, "Proxy has been revoked"));
                };
                value = proxy.target;
            } else {
                return Ok(Value::Boolean(self.objects[handle.slot].length.is_some()));
            }
        }
    }
}
