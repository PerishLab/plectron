use super::{Engine, Expression, Fault, Host, Value, operation};
use crate::syntax::{Body, Item, Key};
use crate::{Error, Failure};

impl Engine {
    pub(crate) fn evaluate(
        &mut self,
        expression: &Expression,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        self.evaluating(|engine| engine.resolve(expression, scope, host))
    }

    pub(super) fn evaluating<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.tick()?;
        if self.depth >= crate::DEPTH {
            return Err(Fault("evaluation depth exhausted".into()).into());
        }
        self.depth += 1;
        let result = operation(self);
        self.depth -= 1;
        result
    }

    fn resolve(
        &mut self,
        expression: &Expression,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        match expression {
            Expression::Await(_) => {
                Err(Fault("await requires a resumable activation".into()).into())
            }
            Expression::Number(value) => Ok(Value::Number(*value)),
            Expression::Text(value) => Ok(Value::Text(value.clone())),
            Expression::Boolean(value) => Ok(Value::Boolean(*value)),
            Expression::Null => Ok(Value::Null),
            Expression::This => self.receiver(scope),
            Expression::Target => Ok(self
                .frame(scope)
                .map(|frame| frame.target.clone())
                .unwrap_or(Value::Undefined)),
            Expression::Parent(arguments) => self.parent(arguments, scope, host),
            Expression::Name(name) => self.lookup(scope, name),
            Expression::Member(..) | Expression::Super(_) => {
                self.reference(expression, scope, host)?.get(self, host)
            }
            Expression::Object(fields) => self.fields(fields, scope, host),
            Expression::Array(items) => self.elements(items, scope, host),
            Expression::Function(function) => Ok(self.closure(function.clone(), scope)),
            Expression::Arrow(function) => Ok(self.arrow(function.clone(), scope)),
            Expression::Class(class) => self.class(class, (scope, None), host),
            Expression::Call(callee, arguments) => self.calling((callee, arguments), scope, host),
            Expression::New(callee, arguments) => {
                let callee = self.evaluate(callee, scope, host)?;
                let values = self.arguments(arguments, scope, host)?;
                self.build(callee, values, host)
            }
            Expression::Unary(operator, expression) => {
                if operator == "delete" {
                    return self.removal(expression, scope, host);
                }
                if operator == "typeof"
                    && let Expression::Name(name) = expression.as_ref()
                    && self.locate(scope, name).is_none()
                {
                    return Ok(Value::Text("undefined".into()));
                }
                let value = self.evaluate(expression, scope, host)?;
                if operator == "typeof" {
                    return Ok(self.kind(&value));
                }
                if matches!(value, Value::Symbol(_)) && operator != "!" {
                    return Err(self.raise(Error::Type, "cannot convert Symbol to number"));
                }
                Ok(operation::unary(operator, value)?)
            }
            Expression::Binary(operator, left, right) => {
                self.binary(operator, (left, right), scope, host)
            }
            Expression::Assign(assignment) => self.assignment(assignment, scope, host),
            Expression::Update(update) => self.updating(update, scope, host),
        }
    }

    pub(crate) fn kind(&self, value: &Value) -> Value {
        let name = match value {
            Value::Undefined => "undefined",
            Value::Null => "object",
            Value::Boolean(_) => "boolean",
            Value::Number(_) => "number",
            Value::Text(_) => "string",
            Value::Symbol(_) => "symbol",
            Value::Object(handle) if self.objects[handle.slot].call.is_some() => "function",
            Value::Object(_) => "object",
        };
        Value::Text(name.into())
    }

    fn removal(
        &mut self,
        expression: &Expression,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        if matches!(expression, Expression::Member(..)) {
            self.reference(expression, scope, host)?.delete(self, host)
        } else {
            self.evaluate(expression, scope, host)?;
            Ok(Value::Boolean(true))
        }
    }

    fn fields(
        &mut self,
        fields: &[(Key, Body)],
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let object = self.object(Default::default())?;
        for (key, body) in fields {
            let name = match key {
                Key::Literal(name) => name.clone().into(),
                Key::Computed(expression) => self.evaluate(expression, scope, host)?.key()?,
            };
            let mut property = match body {
                Body::Field(Some(expression)) => {
                    crate::Property::data(self.named((expression, name.string()), scope, host)?)
                }
                Body::Field(None) => unreachable!(),
                _ => self.method((&object, &name), body, scope)?,
            };
            property.enumerable = true;
            self.install(&object, &name, property)?;
        }
        Ok(object)
    }

    pub(super) fn arguments(
        &mut self,
        arguments: &[Item],
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Vec<Value>, Failure> {
        Ok(self
            .sequence(arguments.iter().map(Some), scope, host)?
            .into_iter()
            .map(|value| value.expect("argument lists have no holes"))
            .collect())
    }

    fn binary(
        &mut self,
        operator: &str,
        pair: (&Expression, &Expression),
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let left = self.evaluate(pair.0, scope, host)?;
        match operator {
            "&&" if !left.truth() => return Ok(left),
            "||" if left.truth() => return Ok(left),
            _ => (),
        }
        let right = self.evaluate(pair.1, scope, host)?;
        if matches!(operator, "instanceof" | "in") {
            return self.relation(operator, left, right, host);
        }
        if !matches!(operator, "===" | "!==" | "&&" | "||")
            && (matches!(left, Value::Symbol(_)) || matches!(right, Value::Symbol(_)))
        {
            return Err(self.raise(Error::Type, "cannot convert Symbol to a primitive operand"));
        }
        Ok(operation::binary(operator, left, right)?)
    }

    fn relation(
        &mut self,
        operator: &str,
        left: Value,
        right: Value,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        if operator == "instanceof" {
            return self.instance(&left, &right, host);
        }
        let key = left.key()?;
        Ok(Value::Boolean(self.checking(&right, &key, host)?))
    }
}
