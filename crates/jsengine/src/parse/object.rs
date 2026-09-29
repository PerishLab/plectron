use super::{Context, Parser, Rejection};
use crate::syntax::{Body, Key};

impl Parser {
    pub(super) fn fields(&mut self) -> Result<Vec<(Key, Body)>, Rejection> {
        let mut fields = Vec::new();
        while !self.eat("}") {
            let mut key = self.key()?;
            let body = if self.eat(":") {
                if matches!(&key, Key::Literal(name) if name == "__proto__") {
                    return Err(self.fault("prototype syntax is unsupported"));
                }
                Body::Field(Some(self.expression(0)?))
            } else {
                let accessor = self.accessor();
                if accessor.is_some() {
                    key = self.key()?;
                }
                self.method(Context::Method, accessor)?
            };
            fields.push((key, body));
            if self.eat("}") {
                break;
            }
            self.expect(",")?;
        }
        Ok(fields)
    }

    pub(super) fn method(
        &mut self,
        context: Context,
        accessor: Option<bool>,
    ) -> Result<Body, Rejection> {
        let function = self.function(context)?;
        match accessor {
            Some(true) if function.params.names().next().is_none() => Ok(Body::Get(function)),
            Some(false) if function.params.items.len() == 1 && function.params.rest.is_none() => {
                Ok(Body::Set(function))
            }
            Some(_) => Err(self.fault("invalid accessor parameter count")),
            None => Ok(Body::Method(function)),
        }
    }
}
