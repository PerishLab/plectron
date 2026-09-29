use super::{
    Web,
    event::{Action, emit, pointer},
    focus::change,
};
use crate::{Fault, error};
use domengine::{Edit, Node};
use jsruntime::Runtime;
mod ime;
pub(super) use ime::abandon;
pub use ime::input as compose;

pub fn edit(runtime: &mut Runtime<Web>, edit: &Edit) -> Result<(), Fault> {
    edit.validate().map_err(error)?;
    if *edit == Edit::Blur {
        return change(runtime, None);
    }
    if *edit == Edit::Next {
        let document = &runtime.addons().document;
        let nodes: Vec<_> = document
            .walk()
            .into_iter()
            .filter(|node| document.focusable(*node))
            .collect();
        let index = nodes
            .iter()
            .position(|node| Some(*node) == document.focused())
            .map_or(0, |index| index + 1);
        let next = nodes.get(index).or_else(|| nodes.first()).copied();
        return change(runtime, next);
    }
    let document = &mut runtime.addons().document;
    let Some(node) = document.focused() else {
        return Ok(());
    };
    if *edit == Edit::Activate {
        if document.activatable(node) {
            emit(runtime, node, "click", None)?;
        }
        return Ok(());
    }
    if !document.editable(node) {
        return Ok(());
    }
    runtime.addons().composition = None;
    if *edit == Edit::Commit {
        return emit(runtime, node, "commit", None).map(|_| ());
    }
    let outcome = if edit.kind().is_some() {
        Some(emit(runtime, node, "beforeinput", Some(edit))?)
    } else {
        None
    };
    if let Some(outcome) = outcome
        && outcome.cancelled
    {
        runtime
            .addons()
            .events
            .action(outcome.id, Action::Prevented);
        return Ok(());
    }
    let web = runtime.addons();
    if web.document.focused() != Some(node) {
        if let Some(outcome) = outcome {
            web.events.action(outcome.id, Action::Retargeted);
        }
        return Ok(());
    }
    let result = web.document.edit(node, edit).map_err(error);
    if let Some(outcome) = outcome {
        match &result {
            Ok(true) => web.events.action(outcome.id, Action::Applied),
            Ok(false) => web.events.action(outcome.id, Action::Unchanged),
            Err(fault) => web.events.fail(outcome.id, fault),
        }
    }
    if result? {
        emit(runtime, node, "input", Some(edit))?;
    }

    Ok(())
}

pub fn transition(
    runtime: &mut Runtime<Web>,
    from: Option<Node>,
    to: Option<Node>,
) -> Result<(), Fault> {
    let path = |runtime: &mut Runtime<Web>, node: Option<Node>| -> Result<Vec<Node>, Fault> {
        let Some(node) = node else {
            return Ok(Vec::new());
        };
        let document = &runtime.addons().document;
        if !document.connected(node).map_err(error)? {
            return Ok(Vec::new());
        }
        document.path(node).map_err(error)
    };
    let from = path(runtime, from)?;
    let to = path(runtime, to)?;
    let shared = from
        .iter()
        .rev()
        .zip(to.iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    for node in from.iter().take(from.len() - shared) {
        pointer(runtime, *node, "pointerleave")?;
    }
    for node in to.iter().take(to.len() - shared).rev() {
        pointer(runtime, *node, "pointerenter")?;
    }
    Ok(())
}
