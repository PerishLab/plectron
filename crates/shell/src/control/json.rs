use std::collections::BTreeMap;

pub const LIMIT: usize = 16_384;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Self::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn boolean(&self) -> Option<bool> {
        match self {
            Self::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    pub fn integer(&self) -> Option<u64> {
        let Self::Number(number) = self else {
            return None;
        };
        if whole(*number) && *number >= 0.0 {
            Some(*number as u64)
        } else {
            None
        }
    }

    pub fn field(&self, key: &str) -> Option<&Value> {
        self.object()?.get(key)
    }
}

pub fn parse(source: &str) -> Result<Value, String> {
    super::parser::parse(source)
}

pub fn encode(value: &Value) -> Result<String, String> {
    let mut output = String::new();
    write(value, &mut output)?;
    Ok(output)
}

fn whole(number: f64) -> bool {
    let finite = number.is_finite() && number.fract() == 0.0;
    finite && (-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(&number)
}

fn write(value: &Value, output: &mut String) -> Result<(), String> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(true) => output.push_str("true"),
        Value::Bool(false) => output.push_str("false"),
        Value::Number(number) => scalar(*number, output)?,
        Value::Text(text) => quote(text, output),
        Value::Array(items) => list(items, output)?,
        Value::Object(entries) => map(entries, output)?,
    }
    Ok(())
}

fn scalar(number: f64, output: &mut String) -> Result<(), String> {
    if !number.is_finite() {
        return Err("non-finite json number".into());
    }
    if whole(number) {
        output.push_str(&(number as i64).to_string());
    } else {
        output.push_str(&number.to_string());
    }
    Ok(())
}

fn list(items: &[Value], output: &mut String) -> Result<(), String> {
    output.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write(item, output)?;
    }
    output.push(']');
    Ok(())
}

fn map(entries: &BTreeMap<String, Value>, output: &mut String) -> Result<(), String> {
    output.push('{');
    for (index, (key, item)) in entries.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        quote(key, output);
        output.push(':');
        write(item, output)?;
    }
    output.push('}');
    Ok(())
}

fn quote(text: &str, output: &mut String) {
    output.push('"');
    for ch in text.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0C}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if (ch as u32) < 0x20 => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output.push('"');
}
