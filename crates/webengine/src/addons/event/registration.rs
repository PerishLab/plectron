use super::super::{Native, text};
use super::{
    Web,
    subscription::{Options, Subscription},
};
use crate::Fault;
use domengine::Node;
use jsengine::{Engine, Value};

impl Web {
    pub(crate) fn listening(
        &mut self,
        engine: &mut Engine,
        node: Node,
        arguments: &[Value],
    ) -> Result<Value, Fault> {
        let [kind, callback, rest @ ..] = arguments else {
            return Err(Fault(
                "expected event name, callback and optional options".into(),
            ));
        };
        let kind = text(kind)?;
        name(kind)?;
        engine.callable(callback)?;
        if rest.len() > 1 {
            return Err(Fault("invalid listener options".into()));
        }
        let options = self.options(engine, rest.first())?;
        let scoped = options.scope != 0;
        let id = self.subscriptions.register(Subscription {
            node,
            kind: kind.into(),
            callback: callback.clone(),
            options,
        })?;
        if kind == "scroll" {
            self.scrolls.observe(node);
        }
        if scoped {
            Ok(Value::Undefined)
        } else {
            self.disposable(engine, Native::Unlisten(id))
        }
    }
    fn options(&self, engine: &Engine, options: Option<&Value>) -> Result<Options, Fault> {
        let scope = options
            .map(|options| engine.get(options, "scope"))
            .transpose()?
            .unwrap_or(Value::Undefined);
        let scope = match scope {
            Value::Undefined => 0,
            Value::Object(handle) => *self
                .scopes
                .get(&handle.id())
                .ok_or_else(|| Fault("foreign subscription scope".into()))?,
            _ => return Err(Fault("foreign subscription scope".into())),
        };
        Ok(Options {
            scope,
            capture: flag(engine, options, "capture")?,
            once: flag(engine, options, "once")?,
            observe: flag(engine, options, "observe")?,
        })
    }
    pub(crate) fn scope(&mut self, engine: &mut Engine) -> Result<Value, Fault> {
        let id = self.subscriptions.scope()?;
        let value = self.disposable(engine, Native::Release(id))?;
        let Value::Object(handle) = value else {
            unreachable!("disposable is an object")
        };
        self.scopes.insert(handle.id(), id);
        self.owners.insert(id, handle.id());
        let value = Value::Object(handle);
        Ok(value)
    }
    fn disposable(&mut self, engine: &mut Engine, native: Native) -> Result<Value, Fault> {
        let dispose = self.bind(engine, native);
        let value = engine.object([("dispose".into(), dispose)].into())?;
        engine.freeze(&value)?;
        Ok(value)
    }
}
pub fn flag(engine: &Engine, options: Option<&Value>, name: &str) -> Result<bool, Fault> {
    match options
        .map(|value| engine.get(value, name))
        .transpose()?
        .unwrap_or(Value::Undefined)
    {
        Value::Undefined => Ok(false),
        Value::Boolean(value) => Ok(value),
        _ => Err(Fault("expected boolean event option".into())),
    }
}
pub fn name(name: &str) -> Result<(), Fault> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b':'))
    {
        return Err(Fault("invalid event name".into()));
    }
    Ok(())
}
