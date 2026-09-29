use crate::value::elements::LIMIT;
use crate::{Access, Addons, Engine, Error, Failure, Invocation, Value};

impl Engine {
    pub(super) fn applying(
        &mut self,
        target: &Value,
        args: &[Value],
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        if self.callable(target).is_err() {
            return Err(self.raise(Error::Type, "Reflect.apply target is not callable"));
        }
        let receiver = args.get(1).cloned().unwrap_or(Value::Undefined);
        let list = args.get(2).cloned().unwrap_or(Value::Undefined);
        let values = self.reflectargs(&list, host)?;
        self.apply(
            target.clone(),
            Invocation {
                receiver,
                arguments: values,
            },
            host,
        )
    }

    pub(super) fn reflectconstruct(
        &mut self,
        target: &Value,
        args: &[Value],
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.construction(target)?;
        let constructor = args.get(2).cloned().unwrap_or_else(|| target.clone());
        self.construction(&constructor)?;
        let list = args.get(1).cloned().unwrap_or(Value::Undefined);
        let values = self.reflectargs(&list, host)?;
        self.buildtarget(target.clone(), constructor, values, host)
    }

    pub(crate) fn reflectargs(
        &mut self,
        list: &Value,
        host: &mut impl Addons,
    ) -> Result<Vec<Value>, Failure> {
        self.validate(list)?;
        if !matches!(list, Value::Object(_)) {
            return Err(self.raise(Error::Type, "arguments list must be an object"));
        }
        let length = self.read(&Access::direct(list, "length"), host)?;
        let number = length.number()?;
        let count = if number.is_nan() || number < 0.0 {
            0
        } else {
            number.floor() as usize
        };
        if count > LIMIT {
            return Err(self.raise(Error::Range, "arguments list is too long"));
        }
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            values.push(self.read(&Access::direct(list, index.to_string()), host)?);
        }
        Ok(values)
    }
}
