use super::{Web, event::emit, event::notification};
use crate::{Fault, error};
use domengine::Node;
use jsengine::Engine;
use jsruntime::Runtime;

pub fn click(runtime: &mut Runtime<Web>, target: Node) -> Result<(), Fault> {
    let focus = runtime
        .addons()
        .document
        .focusable(target)
        .then_some(target);
    change(runtime, focus)?;
    emit(runtime, target, "click", None).map(|_| ())
}

pub fn change(runtime: &mut Runtime<Web>, target: Option<Node>) -> Result<(), Fault> {
    runtime.host(|engine, web| focus(engine, web, target))
}

pub(super) fn focus(engine: &mut Engine, web: &mut Web, target: Option<Node>) -> Result<(), Fault> {
    let previous = web.document.focused();
    web.document.focus(target).map_err(error)?;
    if previous != web.document.focused() {
        super::editing::abandon(web);
    }
    announce(engine, web, previous)
}

pub(super) fn announce(
    engine: &mut Engine,
    web: &mut Web,
    previous: Option<Node>,
) -> Result<(), Fault> {
    let target = web.document.focused();
    if previous == target {
        return Ok(());
    }
    super::editing::abandon(web);
    if let Some(previous) = previous {
        notification(engine, web, previous, "blur")?;
    }
    if web.document.focused() == target
        && let Some(target) = target
    {
        notification(engine, web, target, "focus")?;
    }
    Ok(())
}
