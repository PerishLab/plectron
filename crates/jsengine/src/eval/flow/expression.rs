use super::{
    collect::{Collection, Fields},
    task::{Step, Task},
};
use crate::eval::{
    operation,
    reference::{Key, Reference},
};
use crate::syntax::{Expression, Item};
use crate::{Addons, Engine, Failure, Fault, Value};

impl Task {
    pub(super) fn compute(
        &mut self,
        step: Step,
        engine: &mut Engine,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        match step {
            Step::Expression(expression, scope) => self.expand(expression, scope, engine, host)?,
            Step::Named(expression, scope, name) => {
                if suspends(&expression) {
                    self.expression(expression, scope);
                } else {
                    self.values
                        .push(engine.named((&expression, name.as_deref()), scope, host)?);
                }
            }
            Step::Reference(expression, scope) => match expression {
                Expression::Member(base, key) => {
                    self.steps.push(Step::Property);
                    self.expression(*key, scope);
                    self.expression(*base, scope);
                }
                Expression::Super(key) => {
                    let receiver = engine.receiver(scope)?;
                    self.steps.push(Step::Super(receiver, scope));
                    self.expression(*key, scope);
                }
                expression => self
                    .references
                    .push(engine.reference(&expression, scope, host)?.owned()),
            },
            Step::Property => {
                let key = self.pop();
                let base = self.pop();
                self.references.push(Reference::Property {
                    receiver: base.clone(),
                    base,
                    key: Key::Value(key),
                });
            }
            Step::Super(receiver, scope) => {
                let key = self.pop();
                let home = engine
                    .frame(scope)
                    .and_then(|frame| frame.home.clone())
                    .ok_or_else(|| Fault("missing super home".into()))?;
                let base = engine.parenting(&home, host)?;
                self.references.push(Reference::Property {
                    base,
                    key: Key::Value(key),
                    receiver,
                });
            }
            Step::Get => {
                let reference = self.references.pop().expect("get reference");
                self.values.push(reference.get(engine, host)?);
            }
            Step::Unary(operator) => {
                let value = self.pop();
                let result = if operator == "typeof" {
                    engine.kind(&value)
                } else {
                    operation::unary(&operator, value)?
                };
                self.values.push(result);
            }
            Step::Left(operator, right, scope) => {
                let left = self.pop();
                if (operator == "&&" && !left.truth()) || (operator == "||" && left.truth()) {
                    self.values.push(left);
                } else {
                    self.steps.push(Step::Binary(operator, left));
                    self.expression(right, scope);
                }
            }
            Step::Binary(operator, left) => {
                let right = self.pop();
                let value = if operator == "instanceof" {
                    engine.instance(&left, &right, host)?
                } else {
                    operation::binary(&operator, left, right)?
                };
                self.values.push(value);
            }
            Step::Assignment(operator, value, scope) => {
                let reference = self.references.last().expect("assignment reference");
                let name = naming(reference, &operator);
                let previous = if operator == "=" {
                    None
                } else {
                    Some(reference.get(engine, host)?)
                };
                if previous
                    .as_ref()
                    .is_some_and(|value| skip(&operator, value))
                {
                    self.references.pop();
                    self.values.push(previous.unwrap());
                } else {
                    self.steps.push(Step::Write(operator, previous));
                    self.steps.push(Step::Named(value, scope, name));
                }
            }
            Step::Write(operator, previous) => {
                let value = self.pop();
                let value = assigned(&operator, previous, value)?;
                let reference = self.references.pop().expect("write reference");
                self.values.push(reference.set(engine, value, host)?);
            }
            Step::Update(delta, prefix) => {
                let reference = self.references.pop().expect("update reference");
                let old = reference.get(engine, host)?.number()?;
                let value = Value::Number(old + f64::from(delta));
                reference.set(engine, value.clone(), host)?;
                self.values
                    .push(if prefix { value } else { Value::Number(old) });
            }
            Step::Callee(arguments, scope) => {
                let reference = self.references.pop().expect("call reference");
                let callee = reference.get(engine, host)?;
                self.steps.push(Step::Collect(Collection::call(
                    arguments,
                    scope,
                    (callee, reference.receiver(), false),
                )));
            }
            Step::Callable(arguments, scope, construct) => {
                let callee = self.pop();
                self.steps.push(Step::Collect(Collection::call(
                    arguments,
                    scope,
                    (callee, Value::Undefined, construct),
                )));
            }
            step => self.collect(step, engine, host)?,
        }
        Ok(())
    }
    fn expand(
        &mut self,
        expression: Expression,
        scope: usize,
        engine: &mut Engine,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        if !suspends(&expression) {
            self.values.push(engine.evaluate(&expression, scope, host)?);
            return Ok(());
        }
        match expression {
            Expression::Located(expression, site) => {
                let previous = self.site.replace(site);
                self.steps.push(Step::Site(previous));
                self.expression(*expression, scope);
            }
            Expression::Await(value) => {
                self.steps.push(Step::Await);
                self.expression(*value, scope);
            }
            Expression::Member(..) | Expression::Super(_) => {
                self.steps.push(Step::Get);
                self.steps.push(Step::Reference(expression, scope));
            }
            Expression::Call(callee, arguments) => {
                if matches!(
                    *callee,
                    Expression::Name(_) | Expression::Member(..) | Expression::Super(_)
                ) {
                    self.steps.push(Step::Callee(arguments, scope));
                    self.steps.push(Step::Reference(*callee, scope));
                } else {
                    self.steps.push(Step::Callable(arguments, scope, false));
                    self.expression(*callee, scope);
                }
            }
            Expression::New(callee, arguments) => {
                self.steps.push(Step::Callable(arguments, scope, true));
                self.expression(*callee, scope);
            }
            Expression::Unary(operator, value) => {
                if operator == "typeof"
                    && let Expression::Name(name) = value.as_ref()
                    && engine.locate(scope, name).is_none()
                {
                    self.values.push(Value::Text("undefined".into()));
                    return Ok(());
                }
                self.steps.push(Step::Unary(operator));
                self.expression(*value, scope);
            }
            Expression::Binary(operator, left, right) => {
                self.steps.push(Step::Left(operator, *right, scope));
                self.expression(*left, scope);
            }
            Expression::Assign(assignment) => {
                self.steps.push(Step::Assignment(
                    assignment.operator,
                    *assignment.value,
                    scope,
                ));
                self.steps.push(Step::Reference(*assignment.target, scope));
            }
            Expression::Update(update) => {
                self.steps.push(Step::Update(update.delta, update.prefix));
                self.steps.push(Step::Reference(*update.target, scope));
            }
            Expression::Array(items) => self
                .steps
                .push(Step::Collect(Collection::array(items, scope))),
            Expression::Object(fields) => {
                let object = engine.object(Default::default())?;
                self.steps.push(Step::Fields(Fields {
                    object,
                    fields: fields.into(),
                    scope,
                }));
            }
            _ => return Err(Fault("unsupported suspension expression".into()).into()),
        }
        Ok(())
    }
}
fn skip(operator: &str, value: &Value) -> bool {
    match operator {
        "&&=" => !value.truth(),
        "||=" => value.truth(),
        "??=" => !matches!(value, Value::Null | Value::Undefined),
        _ => false,
    }
}
pub(super) fn suspends(expression: &Expression) -> bool {
    use crate::syntax::{Body, Key};
    match expression {
        Expression::Await(_) => true,
        Expression::Member(a, b) | Expression::Binary(_, a, b) => suspends(a) || suspends(b),
        Expression::Unary(_, value) | Expression::Super(value) | Expression::Located(value, _) => {
            suspends(value)
        }
        Expression::Call(callee, items) | Expression::New(callee, items) => {
            suspends(callee) || items.iter().any(item)
        }
        Expression::Parent(items) => items.iter().any(item),
        Expression::Array(items) => items.iter().flatten().any(item),
        Expression::Object(fields) => fields.iter().any(|(key, body)| {
            matches!(key, Key::Computed(value) if suspends(value))
                || matches!(body, Body::Field(Some(value)) if suspends(value))
        }),
        Expression::Assign(value) => suspends(&value.target) || suspends(&value.value),
        Expression::Update(value) => suspends(&value.target),
        _ => false,
    }
}
fn item(item: &Item) -> bool {
    match item {
        Item::Value(value) | Item::Spread(value) => suspends(value),
    }
}

fn assigned(operator: &str, previous: Option<Value>, value: Value) -> Result<Value, Fault> {
    Ok(match previous {
        Some(previous) if !matches!(operator, "&&=" | "||=" | "??=") => {
            operation::binary(&operator[..operator.len() - 1], previous, value)?
        }
        _ => value,
    })
}

fn naming(reference: &Reference<'_>, operator: &str) -> Option<String> {
    match reference {
        Reference::Binding { name, .. } if matches!(operator, "=" | "&&=" | "||=" | "??=") => {
            Some(name.clone().into_owned())
        }
        _ => None,
    }
}
