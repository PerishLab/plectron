mod elements;
pub(crate) mod error;
mod metadata;
mod object;
pub(crate) use metadata::{Call, Construct, Initializer};
pub(crate) mod promise;
mod property;
mod prototype;
pub(crate) use object::Object;
pub use property::{Content, Property};
pub(crate) mod access;
pub(crate) mod text;
pub use access::Access;
pub(crate) use access::View;
pub(crate) use prototype::Roots;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub(crate) owner: u64,
    pub(crate) slot: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Atom {
    String(String),
    Symbol(Symbol),
}

impl Atom {
    pub(crate) fn string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Symbol(_) => None,
        }
    }
}

impl From<&str> for Atom {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

impl From<String> for Atom {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle {
    pub(crate) owner: u64,
    pub(crate) slot: usize,
    pub(crate) epoch: u64,
}

impl Handle {
    pub fn id(self) -> usize {
        self.slot
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    Text(String),
    Symbol(Symbol),
    Object(Handle),
}

impl Value {
    pub fn truth(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::Text(value) => !value.is_empty(),
            _ => true,
        }
    }

    pub fn text(&self) -> String {
        match self {
            Self::Undefined => "undefined".into(),
            Self::Null => "null".into(),
            Self::Boolean(value) => value.to_string(),
            Self::Number(value) => number(*value),
            Self::Text(value) => value.clone(),
            Self::Symbol(symbol) => format!("Symbol({}:{})", symbol.owner, symbol.slot),
            Self::Object(_) => "[object Object]".into(),
        }
    }

    pub(super) fn number(&self) -> Result<f64, crate::Fault> {
        Ok(match self {
            Self::Null => 0.0,
            Self::Boolean(value) => u8::from(*value).into(),
            Self::Number(value) => *value,
            Self::Text(value) if value.trim().is_empty() => 0.0,
            Self::Text(value) => value.trim().parse().unwrap_or(f64::NAN),
            Self::Symbol(_) => return Err(crate::Fault("cannot convert Symbol to number".into())),
            Self::Object(_) => return Err(crate::Fault("object coercion is unsupported".into())),
            _ => f64::NAN,
        })
    }
}

fn number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "Infinity"
        } else {
            "-Infinity"
        }
        .into();
    }
    value.to_string()
}

impl crate::Engine {
    pub(crate) fn symbols(&mut self) -> Result<(), crate::Fault> {
        let constructor = self.function(Call::Symbol);
        self.signature(&constructor, "Symbol", 0);
        let Value::Object(handle) = constructor else {
            unreachable!()
        };
        self.iterator = self.wellknown(handle, "iterator");
        self.plectron = self.wellknown(handle, "plectron");
        self.register("Symbol", constructor)
    }

    fn wellknown(&mut self, handle: Handle, name: &str) -> Symbol {
        let symbol = Symbol {
            owner: self.identity,
            slot: self.symbols.len(),
        };
        self.symbols.push(Some(format!("Symbol.{name}")));
        self.objects[handle.slot]
            .fields
            .insert(name.into(), Property::fixed(Value::Symbol(symbol), false));
        symbol
    }

    pub(crate) fn symbol(&mut self, arguments: &[Value]) -> Result<Value, crate::Failure> {
        let description = match arguments.first() {
            None | Some(Value::Undefined) => None,
            Some(Value::Symbol(_)) => {
                return Err(self.raise(crate::Error::Type, "cannot convert Symbol to text"));
            }
            Some(value) => Some(value.text()),
        };
        let slot = self.symbols.len();
        self.symbols.push(description);
        Ok(Value::Symbol(Symbol {
            owner: self.identity,
            slot,
        }))
    }
}
