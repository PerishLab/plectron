use super::super::{Native, Web, text};
use crate::{Fault, error};
use domengine::style;
use jsengine::{Engine, Value};

impl Web {
    pub(super) fn styling(
        &mut self,
        engine: &mut Engine,
        native: Native,
        arguments: &[Value],
    ) -> Result<Value, Fault> {
        match (native, arguments) {
            (Native::Get(node), [name]) => {
                let source = self
                    .document
                    .entry(node)
                    .map_err(error)?
                    .attributes
                    .get("style")
                    .cloned()
                    .unwrap_or_default();
                Ok(Value::Text(
                    style::declarations(&source)
                        .map_err(error)?
                        .get(text(name)?)
                        .cloned()
                        .unwrap_or_default(),
                ))
            }
            (Native::Put(node), [name, value]) => {
                let source = self
                    .document
                    .entry(node)
                    .map_err(error)?
                    .attributes
                    .get("style")
                    .cloned()
                    .unwrap_or_default();
                let mut values = style::declarations(&source).map_err(error)?;
                let name = text(name)?;
                let value = text(value)?;
                if value.is_empty() {
                    values.remove(name);
                } else {
                    values.insert(name.into(), value.into());
                }
                let source = values
                    .into_iter()
                    .map(|(name, value)| format!("{name}: {value};"))
                    .collect::<Vec<_>>()
                    .join(" ");
                self.attribute(node, &Value::Text("style".into()), &Value::Text(source))
            }
            (Native::Computed, [node]) => self.computed(engine, self.node(node)?),
            (Native::Keyframes, [name]) => {
                let Some(id) = self.sheet.keyframes(text(name)?).map(|rule| rule.id) else {
                    return Ok(Value::Null);
                };
                if let Some(value) = self.rules.get(&id) {
                    return Ok(value.clone());
                }
                let revision = self.bind(engine, Native::Version(id));
                let opacity = self.bind(engine, Native::Opacity(id));
                let set = self.bind(engine, Native::Change(id));
                let value = engine
                    .object(
                        [
                            ("id".into(), Value::Number(id as f64)),
                            ("revision".into(), revision),
                            ("opacity".into(), opacity),
                            ("setOpacity".into(), set),
                        ]
                        .into(),
                    )
                    .map_err(error)?;
                self.rules.insert(id, value.clone());
                Ok(value)
            }
            (Native::Version(id), []) => {
                let rule = self
                    .sheet
                    .rule(id)
                    .ok_or_else(|| Fault("unknown keyframes rule".into()))?;
                Ok(Value::Number(rule.revision as f64))
            }
            (Native::Opacity(id), [offset]) => {
                let offset = fraction(offset)?;
                let rule = self
                    .sheet
                    .rule(id)
                    .ok_or_else(|| Fault("unknown keyframes rule".into()))?;
                Ok(rule
                    .opacity
                    .iter()
                    .find(|(point, _)| *point == offset)
                    .map_or(Value::Null, |(_, opacity)| {
                        Value::Number(f64::from(*opacity) / 255.0)
                    }))
            }
            (Native::Change(id), [offset, opacity]) => {
                let offset = fraction(offset)?;
                let opacity = (fraction(opacity)? * 255.0).round() as u8;
                self.sheet.set(id, offset, opacity).map_err(error)?;
                Ok(Value::Undefined)
            }
            _ => Err(Fault("invalid style arguments".into())),
        }
    }
}

fn fraction(value: &Value) -> Result<f64, Fault> {
    if let Value::Number(value) = value
        && value.is_finite()
        && (0.0..=1.0).contains(value)
    {
        return Ok(*value);
    }
    Err(Fault("expected a number between zero and one".into()))
}
