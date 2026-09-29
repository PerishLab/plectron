use super::{Context, Expression, Kind, Parser, Rejection, Statement};
use crate::syntax::{Body, Class, Element, Key, Placement};

impl Parser {
    pub(super) fn declaration(&mut self) -> Result<Statement, Rejection> {
        let class = self.class()?;
        let name = class
            .name
            .clone()
            .ok_or_else(|| self.fault("class declaration requires a name"))?;
        Ok(Statement::Declare {
            name,
            mutable: true,
            value: Expression::Class(class),
        })
    }

    pub(super) fn class(&mut self) -> Result<Class, Rejection> {
        let previous = self.asynchronous;
        self.asynchronous = false;
        let result = self.classbody();
        self.asynchronous = previous;
        result
    }
    fn classbody(&mut self) -> Result<Class, Rejection> {
        let name = if self.at("{") || self.at("extends") {
            None
        } else {
            Some(self.name()?)
        };
        let heritage = if self.eat("extends") {
            self.enter()?;
            let heritage = self.left();
            self.depth -= 1;
            Some(Box::new(heritage?))
        } else {
            None
        };
        self.expect("{")?;
        let mut constructor = None;
        let mut elements = Vec::new();
        while !self.eat("}") {
            if self.eat(";") {
                continue;
            }
            let element = self.element(heritage.is_some())?;
            if element.placement == Placement::Instance
                && matches!(&element.key, Key::Literal(name) if name == "constructor")
            {
                let Body::Method(function) = element.body else {
                    return Err(self.fault("constructor field is forbidden"));
                };
                if constructor.replace(function).is_some() {
                    return Err(self.fault("duplicate class constructor"));
                }
            } else {
                elements.push(element);
            }
        }
        Ok(Class {
            name,
            heritage,
            constructor,
            elements,
        })
    }

    fn element(&mut self, derived: bool) -> Result<Element, Rejection> {
        let (placement, mut key) = if self.eat("static") {
            if self.at("(") || self.at("=") || self.at(";") {
                (Placement::Instance, Key::Literal("static".into()))
            } else {
                (Placement::Static, self.key()?)
            }
        } else {
            (Placement::Instance, self.key()?)
        };
        let accessor = self.accessor();
        if accessor.is_some() {
            key = self.key()?;
        }
        if placement == Placement::Static
            && matches!(&key, Key::Literal(name) if name == "prototype")
        {
            return Err(self.fault("static prototype is forbidden"));
        }
        let constructor = matches!(&key, Key::Literal(name) if name == "constructor");
        let body = if self.at("(") {
            let context = if derived && constructor && placement == Placement::Instance {
                Context::Derived
            } else {
                Context::Method
            };
            self.method(context, accessor)?
        } else {
            if accessor.is_some() {
                return Err(self.fault("accessor requires a function body"));
            }
            if constructor {
                return Err(self.fault("constructor field is forbidden"));
            }
            Body::Field(self.initializer()?)
        };
        Ok(Element {
            key,
            placement,
            body,
        })
    }

    pub(super) fn accessor(&self) -> Option<bool> {
        if self.at("(") || self.at("=") || self.at(";") {
            return None;
        }
        match self.tokens.previous().map(|token| &token.kind) {
            Some(Kind::Name(name)) if name == "get" => Some(true),
            Some(Kind::Name(name)) if name == "set" => Some(false),
            _ => None,
        }
    }

    pub(super) fn key(&mut self) -> Result<Key, Rejection> {
        if self.eat("[") {
            let value = self.expression(0)?;
            self.expect("]")?;
            return Ok(Key::Computed(value));
        }
        match self.take() {
            Kind::Name(name) | Kind::Text(name) => Ok(Key::Literal(name)),
            Kind::Number(value) => Ok(Key::Literal(crate::Value::Number(value).text())),
            _ => Err(self.fault("expected method name")),
        }
    }
}
