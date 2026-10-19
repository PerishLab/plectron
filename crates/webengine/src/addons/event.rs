mod dispatch;
mod object;
pub mod record;
mod registration;
mod state;
mod subscription;
use super::Web;
use crate::Fault;
use domengine::Node;
use jsengine::{Engine, Value};
use jsruntime::Runtime;
pub use record::{Action, Origin, Outcome};
pub(super) use state::{Control, Dispatches, Lease};
pub(super) use subscription::Registry;

pub struct Data {
    pub kind: String,
    pub text: String,
    pub input: Option<&'static str>,
    pub bubbles: bool,
    pub cancelable: bool,
    pub origin: Origin,
    pub scroll: Option<crate::change::Scroll>,
    pub position: Option<crate::display::control::Position>,
}
impl Data {
    pub fn host(name: &str, edit: Option<&domengine::Edit>) -> Self {
        Self {
            kind: name.into(),
            text: match edit {
                Some(domengine::Edit::Insert(text)) => text.clone(),
                _ => String::new(),
            },
            input: edit.and_then(domengine::Edit::kind),
            bubbles: true,
            cancelable: name == "beforeinput",
            origin: Origin::Host,
            scroll: None,
            position: None,
        }
    }

    pub fn pointer(name: &str) -> Self {
        Self::notification(name)
    }

    pub fn notification(name: &str) -> Self {
        Self {
            kind: name.into(),
            text: String::new(),
            input: None,
            bubbles: false,
            cancelable: false,
            origin: Origin::Host,
            scroll: None,
            position: None,
        }
    }

    pub fn composition(text: &str) -> Self {
        Self {
            kind: "input".into(),
            text: text.into(),
            input: Some("insertCompositionText"),
            bubbles: true,
            cancelable: false,
            origin: Origin::Host,
            scroll: None,
            position: None,
        }
    }
}
pub(super) fn composition(
    engine: &mut Engine,
    web: &mut Web,
    target: Node,
    text: &str,
) -> Result<Outcome, Fault> {
    dispatch::emit(engine, web, target, Data::composition(text))
}
pub(super) fn emit(
    runtime: &mut Runtime<Web>,
    target: Node,
    name: &str,
    edit: Option<&domengine::Edit>,
) -> Result<Outcome, Fault> {
    runtime.host(|engine, web| dispatch::emit(engine, web, target, Data::host(name, edit)))
}
pub(super) fn pointer(
    runtime: &mut Runtime<Web>,
    target: Node,
    name: &str,
) -> Result<Outcome, Fault> {
    runtime.host(|engine, web| dispatch::emit(engine, web, target, Data::pointer(name)))
}
pub(crate) fn notification(
    engine: &mut Engine,
    web: &mut Web,
    target: Node,
    name: &str,
) -> Result<Outcome, Fault> {
    dispatch::emit(engine, web, target, Data::notification(name))
}
pub(crate) fn scroll(
    engine: &mut Engine,
    web: &mut Web,
    change: crate::display::Change,
) -> Result<Outcome, Fault> {
    let target = change
        .node
        .ok_or_else(|| Fault("scroll event has no target".into()))?;
    let mut data = Data::notification("scroll");
    data.scroll = Some(change.fact);
    data.position = change.position;
    dispatch::emit(engine, web, target, data)
}
pub fn message(runtime: &mut Runtime<Web>, message: String) -> Result<(), Fault> {
    if let Some(callback) = runtime.addons().receiver.clone() {
        runtime
            .call(callback, vec![Value::Text(message)])
            .map_err(|failure| runtime.diagnostic(&failure))?;
    }
    Ok(())
}

impl Web {
    pub(super) fn dispatch(
        &mut self,
        engine: &mut Engine,
        node: Node,
        arguments: &[Value],
    ) -> Result<Value, Fault> {
        let [name, text, rest @ ..] = arguments else {
            return Err(Fault(
                "expected event name, text and optional policy".into(),
            ));
        };
        let kind = super::text(name)?;
        registration::name(kind)?;
        let text = super::text(text)?;
        if text.len() > 4096 || rest.len() > 1 {
            return Err(Fault("invalid application event arguments".into()));
        }
        let options = rest.first();
        let data = Data {
            kind: kind.into(),
            text: text.into(),
            input: None,
            origin: Origin::Script,
            bubbles: registration::flag(engine, options, "bubbles")?,
            cancelable: registration::flag(engine, options, "cancelable")?,
            scroll: None,
            position: None,
        };
        let outcome = dispatch::emit(engine, self, node, data)?;
        let result =
            engine.object([("cancelled".into(), Value::Boolean(outcome.cancelled))].into())?;
        engine.freeze(&result)?;
        Ok(result)
    }
}
