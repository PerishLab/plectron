use super::{
    Data, Web,
    record::Phase,
    state::{Control, Lease},
};
use crate::Fault;
use domengine::Node;
use jsengine::{Engine, Value};

pub fn event(
    engine: &mut Engine,
    web: &mut Web,
    target: Node,
    data: &Data,
) -> Result<Value, Fault> {
    let target = web.wrap(engine, target)?;
    let scroll = data
        .scroll
        .as_ref()
        .map(|change| scrolling(engine, web, change, data.position))
        .transpose()?
        .unwrap_or(Value::Undefined);
    let fields = [
        ("type".into(), Value::Text(data.kind.clone())),
        ("target".into(), target),
        ("data".into(), Value::Text(data.text.clone())),
        (
            "inputType".into(),
            data.input
                .map_or(Value::Undefined, |kind| Value::Text(kind.into())),
        ),
        ("bubbles".into(), Value::Boolean(data.bubbles)),
        ("cancelable".into(), Value::Boolean(data.cancelable)),
        ("scroll".into(), scroll),
    ]
    .into();
    let value = engine.object(fields)?;
    engine.freeze(&value)?;
    Ok(value)
}

fn scrolling(
    engine: &mut Engine,
    web: &mut Web,
    change: &crate::change::Scroll,
    position: Option<crate::display::control::Position>,
) -> Result<Value, Fault> {
    let causes = engine.array(
        change
            .causes
            .iter()
            .map(|cause| Some(Value::Text(cause.name().into())))
            .collect(),
    )?;
    engine.freeze(&causes)?;
    let position = if let Some(position) = position {
        let mut values = Vec::new();
        for bookmark in position.fallbacks.into_iter().flatten() {
            let target = web.wrap(engine, bookmark.target)?;
            let value = engine.object(
                [
                    ("target".into(), target),
                    ("inset".into(), Value::Number(bookmark.inset as f64)),
                ]
                .into(),
            )?;
            engine.freeze(&value)?;
            values.push(Some(value));
        }
        let fallbacks = engine.array(values)?;
        engine.freeze(&fallbacks)?;
        let target = web.wrap(engine, position.anchor.target)?;
        let value = engine.object(
            [
                ("target".into(), target),
                ("inset".into(), Value::Number(position.anchor.inset as f64)),
                ("fallbacks".into(), fallbacks),
            ]
            .into(),
        )?;
        engine.freeze(&value)?;
        value
    } else {
        Value::Null
    };
    let value = engine.object(
        [
            ("previous".into(), Value::Number(change.previous as f64)),
            ("offset".into(), Value::Number(change.offset as f64)),
            ("range".into(), Value::Number(change.range as f64)),
            ("edge".into(), Value::Text(change.edge.name().into())),
            ("cause".into(), Value::Text(change.cause.name().into())),
            ("causes".into(), causes),
            ("revision".into(), Value::Number(change.revision as f64)),
            ("position".into(), position),
        ]
        .into(),
    )?;
    engine.freeze(&value)?;
    Ok(value)
}
pub struct Context {
    pub node: Node,
    pub phase: Phase,
    pub lease: Lease,
    pub observe: bool,
}
pub fn context(engine: &mut Engine, web: &mut Web, context: Context) -> Result<Value, Fault> {
    let target = web.wrap(engine, context.node)?;
    let mut fields = std::collections::BTreeMap::from([
        ("currentTarget".into(), target),
        ("phase".into(), Value::Text(context.phase.name().into())),
    ]);
    if !context.observe {
        for (name, control) in [
            ("stopPropagation", Control::Stop),
            ("stopImmediatePropagation", Control::Immediate),
            ("preventDefault", Control::Cancel),
        ] {
            fields.insert(
                name.into(),
                web.bind(
                    engine,
                    super::super::Native::Control(context.lease, control),
                ),
            );
        }
    }
    let value = engine.object(fields)?;
    engine.freeze(&value)?;
    Ok(value)
}
