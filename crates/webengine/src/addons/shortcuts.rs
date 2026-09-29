use super::{Native, Web, text};
use crate::{
    Fault, error,
    keyboard::{self, Binding, Chord, Context, Handle, Set},
};
use jsengine::{Engine, Value};

impl Web {
    pub(super) fn shortcuts(&mut self, engine: &mut Engine) -> Result<Value, Fault> {
        let register = self.bind(engine, Native::Register);
        let value = engine.object([("register".into(), register)].into())?;
        engine.freeze(&value)?;
        Ok(value)
    }

    pub(super) fn register(
        &mut self,
        engine: &mut Engine,
        arguments: &[Value],
    ) -> Result<Value, Fault> {
        let [context, priority, entries] = arguments else {
            return Err(Fault("expected scope, priority and binding records".into()));
        };
        let context = Context::parse(text(context)?)?;
        let set = parse(engine, priority, entries)?;
        let handle = self.keyboard.scope(context).register(set).map_err(error)?;
        self.ticket(engine, context, handle)
    }

    fn ticket(
        &mut self,
        engine: &mut Engine,
        context: Context,
        handle: Handle,
    ) -> Result<Value, Fault> {
        let replace = self.bind(engine, Native::Rebind(context, handle.clone()));
        let dispose = self.bind(engine, Native::Dispose(context, handle));
        let value =
            engine.object([("replace".into(), replace), ("dispose".into(), dispose)].into())?;
        engine.freeze(&value)?;
        Ok(value)
    }
}

pub(super) fn parse(
    engine: &Engine,
    priority: &Value,
    entries: &Value,
) -> Result<Set<crate::Edit>, Fault> {
    let Value::Number(priority) = priority else {
        return Err(Fault("expected integer priority".into()));
    };
    if !(i32::MIN as f64..=i32::MAX as f64).contains(priority) || priority.fract() != 0.0 {
        return Err(Fault("expected integer priority".into()));
    }
    let Value::Number(length) = engine.get(entries, "length")? else {
        return Err(Fault("expected binding list".into()));
    };
    if !(0.0..=64.0).contains(&length) || length.fract() != 0.0 {
        return Err(Fault("binding list exceeds budget".into()));
    }
    let bindings = (0..length as usize)
        .map(|index| {
            let entry = engine.get(entries, &index.to_string())?;
            binding(engine, &entry)
        })
        .collect::<Result<_, Fault>>()?;
    Ok(Set {
        priority: *priority as i32,
        bindings,
    })
}

fn binding(engine: &Engine, entry: &Value) -> Result<Binding<crate::Edit>, Fault> {
    Ok(Binding {
        chord: Chord::parse(text(&engine.get(entry, "chord")?)?).map_err(error)?,
        command: keyboard::command(text(&engine.get(entry, "command")?)?)?,
        repeat: boolean(engine.get(entry, "repeat")?, false)?,
        consume: boolean(engine.get(entry, "consume")?, true)?,
    })
}
fn boolean(value: Value, default: bool) -> Result<bool, Fault> {
    match value {
        Value::Boolean(value) => Ok(value),
        Value::Undefined => Ok(default),
        _ => Err(Fault("expected boolean binding option".into())),
    }
}
