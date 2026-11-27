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
    Session(runtime).run(edit)
}

struct Session<'a>(&'a mut Runtime<Web>);

impl Session<'_> {
    fn run(&mut self, edit: &Edit) -> Result<(), Fault> {
        edit.validate().map_err(error)?;
        if *edit == Edit::Blur {
            return change(self.0, None);
        }
        if *edit == Edit::Next {
            return self.advance();
        }
        let Some(node) = self.0.addons().document.focused() else {
            return Ok(());
        };
        if *edit == Edit::Activate {
            if self.0.addons().document.activatable(node) {
                emit(self.0, node, "click", None)?;
            }
            return Ok(());
        }
        if !self.0.addons().document.editable(node) {
            return Ok(());
        }
        self.0.addons().composition = None;
        if *edit == Edit::Commit {
            return emit(self.0, node, "commit", None).map(|_| ());
        }
        if *edit == Edit::Copy {
            return self.copy(node);
        }
        let Some((resolved, copied)) = self.prepare(node, edit)? else {
            return Ok(());
        };
        self.apply(node, &resolved, copied.as_deref())
    }

    fn copy(&mut self, node: Node) -> Result<(), Fault> {
        let selected = self
            .0
            .addons()
            .document
            .buffer(node)
            .map_err(error)?
            .selected()
            .to_owned();
        if !selected.is_empty() {
            self.0.addons().keyboard.clipboard.write(&selected)?;
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), Fault> {
        let document = &self.0.addons().document;
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
        change(self.0, next)
    }

    fn payload(&mut self, source: &Option<String>) -> Result<Option<String>, Fault> {
        if let Some(text) = source {
            return Ok(Some(text.clone()));
        }
        Ok(self
            .0
            .addons()
            .keyboard
            .clipboard
            .read()?
            .filter(|text| !text.is_empty()))
    }

    fn prepare(
        &mut self,
        node: Node,
        edit: &Edit,
    ) -> Result<Option<(Edit, Option<String>)>, Fault> {
        let buffer = self.0.addons().document.buffer(node).map_err(error)?;
        buffer.check(edit).map_err(error)?;
        match edit {
            Edit::Newline if !buffer.multiline() => Ok(None),
            Edit::Cut => {
                let selected = buffer.selected().to_owned();
                Ok((!selected.is_empty()).then_some((Edit::Cut, Some(selected))))
            }
            Edit::Paste(source) => {
                let empty = buffer.selection().range().is_empty();
                let Some(raw) = self.payload(source)? else {
                    return Ok(None);
                };
                let cleaned = self
                    .0
                    .addons()
                    .document
                    .buffer(node)
                    .map_err(error)?
                    .sanitize(&raw);
                if cleaned.is_empty() && empty {
                    return Ok(None);
                }
                Ok(Some((Edit::Paste(Some(cleaned)), None)))
            }
            _ => Ok(Some((edit.clone(), None))),
        }
    }

    fn apply(&mut self, node: Node, edit: &Edit, copied: Option<&str>) -> Result<(), Fault> {
        let outcome = if edit.kind().is_some() {
            Some(emit(self.0, node, "beforeinput", Some(edit))?)
        } else {
            None
        };
        if let Some(outcome) = outcome
            && outcome.cancelled
        {
            self.0.addons().events.action(outcome.id, Action::Prevented);
            return Ok(());
        }
        let web = self.0.addons();
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
            if let Some(text) = copied {
                self.0.addons().keyboard.clipboard.write(text)?;
            }
            emit(self.0, node, "input", Some(edit))?;
        }
        Ok(())
    }
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
