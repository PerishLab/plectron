use super::json::{LIMIT, Value};
use std::collections::BTreeMap;

const DEPTH: usize = 32;

pub(crate) fn parse(source: &str) -> Result<Value, String> {
    if source.len() > LIMIT {
        return Err("json payload exceeds byte budget".into());
    }
    let mut parser = Parser {
        bytes: source.as_bytes(),
        index: 0,
        depth: 0,
    };
    let value = parser.value()?;
    parser.skip();
    if parser.index != parser.bytes.len() {
        return Err("trailing characters in json payload".into());
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        while let Some(byte) = self.bytes.get(self.index) {
            if matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn take(&mut self) -> Result<u8, String> {
        let byte = self
            .peek()
            .ok_or_else(|| "unexpected end of json payload".to_string())?;
        self.index += 1;
        Ok(byte)
    }

    fn value(&mut self) -> Result<Value, String> {
        self.skip();
        match self.peek() {
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'"') => self.string().map(Value::Text),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err("invalid json value".into()),
        }
    }

    fn literal(&mut self, expected: &[u8], value: Value) -> Result<Value, String> {
        let end = self.index + expected.len();
        if self.bytes.get(self.index..end) != Some(expected) {
            return Err("invalid json literal".into());
        }
        self.index = end;
        Ok(value)
    }

    fn array(&mut self) -> Result<Value, String> {
        self.enter()?;
        self.index += 1;
        let mut items = Vec::new();
        self.skip();
        if self.peek() == Some(b']') {
            self.index += 1;
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip();
            match self.take()? {
                b',' => (),
                b']' => break,
                _ => return Err("expected comma or closing bracket in json array".into()),
            }
        }
        self.depth -= 1;
        Ok(Value::Array(items))
    }

    fn object(&mut self) -> Result<Value, String> {
        self.enter()?;
        self.index += 1;
        let mut entries = BTreeMap::new();
        self.skip();
        if self.peek() == Some(b'}') {
            self.index += 1;
            self.depth -= 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip();
            let key = self.string()?;
            self.skip();
            if self.take()? != b':' {
                return Err("expected colon in json object".into());
            }
            let value = self.value()?;
            if entries.insert(key, value).is_some() {
                return Err("duplicate key in json object".into());
            }
            self.skip();
            match self.take()? {
                b',' => (),
                b'}' => break,
                _ => return Err("expected comma or closing brace in json object".into()),
            }
        }
        self.depth -= 1;
        Ok(Value::Object(entries))
    }

    fn enter(&mut self) -> Result<(), String> {
        if self.depth >= DEPTH {
            return Err("json nesting depth exhausted".into());
        }
        self.depth += 1;
        Ok(())
    }

    fn string(&mut self) -> Result<String, String> {
        if self.take()? != b'"' {
            return Err("expected json string".into());
        }
        let mut output = String::new();
        while self.index < self.bytes.len() {
            match self.bytes[self.index] {
                b'"' => {
                    self.index += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.index += 1;
                    output.push(self.escape()?);
                }
                0..=0x1f => return Err("unescaped control character in json string".into()),
                _ => output.push(self.scalar()?),
            }
        }
        Err("unterminated json string".into())
    }

    fn scalar(&mut self) -> Result<char, String> {
        let text = std::str::from_utf8(&self.bytes[self.index..])
            .map_err(|_| "invalid utf8 in json string".to_string())?;
        let ch = text
            .chars()
            .next()
            .ok_or_else(|| "truncated json string".to_string())?;
        self.index += ch.len_utf8();
        Ok(ch)
    }

    fn escape(&mut self) -> Result<char, String> {
        match self.take()? {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{08}'),
            b'f' => Ok('\u{0C}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.unicode(),
            _ => Err("invalid json escape sequence".into()),
        }
    }

    fn unicode(&mut self) -> Result<char, String> {
        let first = self.hex()?;
        if !(0xD800..=0xDFFF).contains(&first) {
            return char::from_u32(u32::from(first))
                .ok_or_else(|| "invalid unicode scalar in json".to_string());
        }
        if first > 0xDBFF {
            return Err("unexpected low surrogate in json".into());
        }
        if self.take()? != b'\\' || self.take()? != b'u' {
            return Err("expected low surrogate pair in json".into());
        }
        let second = self.hex()?;
        if !(0xDC00..=0xDFFF).contains(&second) {
            return Err("invalid low surrogate in json".into());
        }
        let combined = 0x10000 + ((u32::from(first - 0xD800) << 10) | u32::from(second - 0xDC00));
        char::from_u32(combined).ok_or_else(|| "invalid surrogate scalar in json".to_string())
    }

    fn hex(&mut self) -> Result<u16, String> {
        let mut value = 0u16;
        for _ in 0..4 {
            let digit = match self.take()? {
                b @ b'0'..=b'9' => u16::from(b - b'0'),
                b @ b'a'..=b'f' => u16::from(b - b'a' + 10),
                b @ b'A'..=b'F' => u16::from(b - b'A' + 10),
                _ => return Err("invalid hex escape in json".into()),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.index;
        if self.peek() == Some(b'-') {
            self.index += 1;
        }
        match self.peek() {
            Some(b'0') => self.index += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err("invalid json number".into()),
        }
        if self.peek() == Some(b'.') {
            self.index += 1;
            let fraction = self.index;
            self.digits();
            if self.index == fraction {
                return Err("invalid json fraction".into());
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            let exponent = self.index;
            self.digits();
            if self.index == exponent {
                return Err("invalid json exponent".into());
            }
        }
        let slice = std::str::from_utf8(&self.bytes[start..self.index])
            .map_err(|_| "invalid json number utf8".to_string())?;
        let parsed: f64 = slice
            .parse()
            .map_err(|_| "json number out of range".to_string())?;
        if !parsed.is_finite() {
            return Err("non-finite json number".into());
        }
        Ok(Value::Number(parsed))
    }

    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.index += 1;
        }
    }
}
