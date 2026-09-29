use super::Call;
use crate::{Engine, Error, Failure, Fault, Invocation, Value};

#[derive(Clone, Copy)]
pub(crate) enum Method {
    At,
    Slice,
    Point,
}

impl Engine {
    pub(crate) fn strings(&mut self) {
        for (name, method, length) in [
            ("at", Method::At, 1),
            ("slice", Method::Slice, 2),
            ("codePointAt", Method::Point, 1),
        ] {
            let function = self.function(Call::Text(method));
            self.signature(&function, name, length);
            self.strings.insert(name, function);
        }
    }

    pub(crate) fn text(&self, text: &str, key: &str) -> Value {
        if key == "length" {
            return Value::Number(text.chars().count() as f64);
        }
        if let Some(value) = self.strings.get(key) {
            return value.clone();
        }
        key.parse::<usize>()
            .ok()
            .filter(|index| index.to_string() == key)
            .and_then(|index| text.chars().nth(index))
            .map(|value| Value::Text(value.to_string()))
            .unwrap_or(Value::Undefined)
    }

    pub(crate) fn string(
        &mut self,
        method: Method,
        invocation: Invocation,
    ) -> Result<Value, Failure> {
        let Value::Text(text) = invocation.receiver else {
            return Err(self.raise(Error::Type, "string method requires a text receiver"));
        };
        Ok(method.run(&text, &invocation.arguments)?)
    }
}

impl Method {
    fn run(self, text: &str, arguments: &[Value]) -> Result<Value, Fault> {
        let length = text.chars().count() as f64;
        let start = position(arguments.first(), 0.0)?;
        match self {
            Self::At => Ok(character(text, relative(start, length), length)
                .map(|value| Value::Text(value.to_string()))
                .unwrap_or(Value::Undefined)),
            Self::Point => Ok(character(text, start, length)
                .map(|value| Value::Number(u32::from(value).into()))
                .unwrap_or(Value::Undefined)),
            Self::Slice => {
                let end = position(arguments.get(1), length)?;
                let start = relative(start, length).clamp(0.0, length) as usize;
                let end = relative(end, length).clamp(0.0, length) as usize;
                Ok(Value::Text(
                    text.chars()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .collect(),
                ))
            }
        }
    }
}

fn position(value: Option<&Value>, default: f64) -> Result<f64, Fault> {
    let Some(value) = value.filter(|value| **value != Value::Undefined) else {
        return Ok(default);
    };
    let value = value.number()?;
    Ok(if value.is_nan() { 0.0 } else { value.trunc() })
}

fn relative(index: f64, length: f64) -> f64 {
    if index < 0.0 { length + index } else { index }
}

fn character(text: &str, index: f64, length: f64) -> Option<char> {
    if !index.is_finite() || index < 0.0 || index >= length {
        return None;
    }
    text.chars().nth(index as usize)
}
