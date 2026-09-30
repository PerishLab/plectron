mod initializer;

use crate::syntax::{Body, Class, Element, Expression, Key, Placement};
use crate::value::{Call, Construct, Initializer};
use crate::{Addons, Engine, Error, Failure, Value};

impl Engine {
    pub(super) fn named(
        &mut self,
        input: (&Expression, Option<&str>),
        scope: usize,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        if let Expression::Located(expression, site) = input.0 {
            let previous = self.site.replace(site.clone());
            let result = self.named((expression, input.1), scope, host);
            self.site = previous;
            return result.map_err(|failure| failure.at(Some(site.clone())));
        }
        if let Expression::Class(class) = input.0 {
            self.tick()?;
            return self.class(class, (scope, input.1), host);
        }
        let value = self.evaluate(input.0, scope, host)?;
        if let Expression::Function(function) | Expression::Arrow(function) = input.0
            && let Some(name) = input.1
        {
            self.signature(&value, name, function.params.arity());
        }
        Ok(value)
    }

    pub(super) fn class(
        &mut self,
        class: &Class,
        context: (usize, Option<&str>),
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let scope = self.scope(context.0);
        if let Some(name) = &class.name {
            self.reserve(scope, name, false)?;
        }
        let parents = self.heritage(class.heritage.as_deref(), scope, host)?;
        let prototype = self.ordinary(Default::default());
        self.inherit(&prototype, Some(&parents.1));
        let construct = self.implementation(class, scope, &prototype);
        let constructor = self.constructor(Call::Class, construct, prototype.clone());
        self.inherit(&constructor, Some(&parents.0));
        let name = class.name.as_deref().or(context.1).unwrap_or("");
        let length = class
            .constructor
            .as_ref()
            .map_or(0, |function| function.params.arity());
        self.signature(&constructor, name, length);
        let definition = Definition {
            constructor,
            prototype,
            scope,
        };
        let mut initializers = Vec::new();
        for element in &class.elements {
            if let Some(initializer) = self.element(element, &definition, host)? {
                initializers.push(initializer);
            }
        }
        if let Some(name) = &class.name {
            self.initialize(scope, name, definition.constructor.clone());
        }
        self.populate(&definition.constructor, &initializers, host)?;
        Ok(definition.constructor)
    }

    fn heritage(
        &mut self,
        expression: Option<&Expression>,
        scope: usize,
        host: &mut impl Addons,
    ) -> Result<(Value, Value), Failure> {
        let function = Value::Object(self.roots.function);
        let Some(expression) = expression else {
            return Ok((function, Value::Object(self.roots.object)));
        };
        let parent = self.evaluate(expression, scope, host)?;
        if parent == Value::Null {
            return Ok((function, Value::Null));
        }
        self.construction(&parent)?;
        let prototype = self.read(&crate::Access::direct(&parent, "prototype"), host)?;
        if !matches!(prototype, Value::Object(_) | Value::Null) {
            return Err(self.raise(Error::Type, "heritage prototype must be an object or null"));
        }
        Ok((parent, prototype))
    }

    fn implementation(&mut self, class: &Class, scope: usize, home: &Value) -> Construct {
        if class.heritage.is_some() && class.constructor.is_none() {
            return Construct::Derived(None);
        }
        let slot = self.capture(class.constructor.clone().unwrap_or_default(), scope);
        self.closures[slot].home = Some(home.clone());
        if class.heritage.is_some() {
            Construct::Derived(Some(slot))
        } else {
            Construct::Closure(slot)
        }
    }

    fn element(
        &mut self,
        element: &Element,
        definition: &Definition,
        host: &mut impl Addons,
    ) -> Result<Option<Initializer>, Failure> {
        let name = match &element.key {
            Key::Literal(name) => name.clone().into(),
            Key::Computed(value) => self.evaluate(value, definition.scope, host)?.key()?,
        };
        let home = match element.placement {
            Placement::Static => &definition.constructor,
            Placement::Instance => &definition.prototype,
        };
        match &element.body {
            Body::Method(_) | Body::Get(_) | Body::Set(_) => {
                let property = self.method((home, &name), &element.body, definition.scope)?;
                self.install(home, &name, property)?;
            }
            Body::Field(expression) => {
                let initializer = Initializer {
                    name,
                    expression: expression.clone(),
                    scope: definition.scope,
                    home: home.clone(),
                };
                if element.placement == Placement::Static {
                    return Ok(Some(initializer));
                }
                let Value::Object(handle) = &definition.constructor else {
                    unreachable!()
                };
                self.objects[handle.slot].initializers.push(initializer);
            }
        }
        Ok(None)
    }
}

struct Definition {
    constructor: Value,
    prototype: Value,
    scope: usize,
}
