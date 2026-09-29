use crate::syntax::Body;
use crate::value::{Atom, Call};
use crate::{Content, Engine, Failure, Property, Value};

impl Engine {
    pub(crate) fn method(
        &mut self,
        target: (&Value, &Atom),
        body: &Body,
        scope: usize,
    ) -> Result<Property, Failure> {
        let (function, prefix) = match body {
            Body::Method(function) => (function, ""),
            Body::Get(function) => (function, "get "),
            Body::Set(function) => (function, "set "),
            Body::Field(_) => unreachable!(),
        };
        let slot = self.capture(function.clone(), scope);
        self.closures[slot].home = Some(target.0.clone());
        let value = self.function(Call::Closure(slot));
        self.signature(
            &value,
            &format!("{prefix}{}", target.1.string().unwrap_or("[symbol]")),
            function.params.arity(),
        );
        if matches!(body, Body::Method(_)) {
            return Ok(Property::hidden(value));
        }
        let (mut get, mut set) = match self
            .inspect(target.0, target.1)?
            .map(|property| property.content)
        {
            Some(Content::Accessor { get, set }) => (get, set),
            _ => (None, None),
        };
        match body {
            Body::Get(_) => get = Some(value),
            Body::Set(_) => set = Some(value),
            _ => unreachable!(),
        }
        Ok(Property {
            content: Content::Accessor { get, set },
            enumerable: false,
            configurable: true,
        })
    }
}
