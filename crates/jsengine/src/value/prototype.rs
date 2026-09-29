use super::{Call, Object, Property};
use crate::{Engine, Error, Failure, Fault, Handle, Value};

pub(crate) struct Roots {
    pub object: Handle,
    pub function: Handle,
    pub array: Handle,
    pub iterator: Handle,
}

impl Roots {
    pub fn new(owner: u64) -> Self {
        Self {
            object: Handle {
                owner,
                slot: 0,
                epoch: 0,
            },
            function: Handle {
                owner,
                slot: 1,
                epoch: 0,
            },
            array: Handle {
                owner,
                slot: 2,
                epoch: 0,
            },
            iterator: Handle {
                owner,
                slot: 3,
                epoch: 0,
            },
        }
    }

    pub fn objects(&self) -> Vec<Object> {
        let object = Object::new(Default::default());
        let mut function = Object::new(Default::default());
        function.prototype = Some(self.object);
        function.call = Some(Call::Empty);
        function.fields.insert(
            "name".into(),
            Property::constant(Value::Text(String::new())),
        );
        function
            .fields
            .insert("length".into(), Property::constant(Value::Number(0.0)));
        let mut array = Object::new(Default::default());
        array.prototype = Some(self.object);
        let mut iterator = Object::new(Default::default());
        iterator.prototype = Some(self.object);
        vec![object, function, array, iterator]
    }
}

impl Engine {
    pub(crate) fn functions(&mut self) {
        let call = self.function(Call::Invoke);
        self.signature(&call, "call", 1);
        self.objects[self.roots.function.slot]
            .fields
            .insert("call".into(), Property::hidden(call));
    }
    pub fn prototype(&self, value: &Value) -> Result<Value, Fault> {
        self.guard(value)?;
        let Value::Object(handle) = value else {
            return Err(Fault("expected object".into()));
        };
        Ok(self.objects[handle.slot]
            .prototype
            .map(Value::Object)
            .unwrap_or(Value::Null))
    }

    pub fn rebase(&mut self, value: &Value, parent: &Value) -> Result<bool, Failure> {
        self.guard(value)?;
        self.guard(parent)?;
        let Value::Object(handle) = value else {
            return Err(self.raise(Error::Type, "expected object"));
        };
        let next = match parent {
            Value::Object(parent) => Some(*parent),
            Value::Null => None,
            _ => return Err(self.raise(Error::Type, "prototype must be an object or null")),
        };
        if self.objects[handle.slot].prototype == next {
            return Ok(true);
        }
        if !self.objects[handle.slot].extensible {
            return Ok(false);
        }
        let mut cursor = next;
        for _ in 0..self.objects.len() {
            let Some(current) = cursor else {
                self.objects[handle.slot].prototype = next;
                return Ok(true);
            };
            if current == *handle {
                return Ok(false);
            }
            cursor = self.objects[current.slot].prototype;
        }
        Ok(false)
    }

    pub(crate) fn inherit(&mut self, value: &Value, parent: Option<&Value>) {
        if let Value::Object(handle) = value
            && let Some(parent) = parent
        {
            self.objects[handle.slot].prototype = match parent {
                Value::Object(parent) => Some(*parent),
                Value::Null => None,
                _ => unreachable!(),
            };
        }
    }

    pub(crate) fn creation(&mut self, constructor: &Value) -> Result<Value, Fault> {
        let parent = match self.get(constructor, "prototype")? {
            Value::Object(handle) => handle,
            _ => self.roots.object,
        };
        let mut object = Object::new(Default::default());
        object.prototype = Some(parent);
        Ok(self.allocate(object))
    }

    pub(crate) fn creating(
        &mut self,
        constructor: &Value,
        host: &mut impl crate::Addons,
    ) -> Result<Value, Failure> {
        let parent = match self.read(&crate::Access::direct(constructor, "prototype"), host)? {
            Value::Object(handle) => handle,
            _ => self.roots.object,
        };
        let mut object = Object::new(Default::default());
        object.prototype = Some(parent);
        Ok(self.allocate(object))
    }

    pub(crate) fn instance(
        &mut self,
        value: &Value,
        constructor: &Value,
        host: &mut impl crate::Addons,
    ) -> Result<Value, Failure> {
        self.validate(value)?;
        self.validate(constructor)?;
        if self.callable(constructor).is_err() {
            return Err(self.raise(Error::Type, "instanceof target is not callable"));
        }
        if !matches!(value, Value::Object(_)) {
            return Ok(Value::Boolean(false));
        }
        let prototype = self.read(&crate::Access::direct(constructor, "prototype"), host)?;
        if !matches!(prototype, Value::Object(_)) {
            return Err(self.raise(Error::Type, "constructor prototype is not an object"));
        }
        let mut current = self.parenting(value, host)?;
        while let Value::Object(_) = current {
            if current == prototype {
                return Ok(Value::Boolean(true));
            }
            current = self.parenting(&current, host)?;
        }
        Ok(Value::Boolean(false))
    }
}
