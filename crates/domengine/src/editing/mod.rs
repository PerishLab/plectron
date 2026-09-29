mod buffer;
pub(crate) mod ime;
mod selection;
use crate::change::Fact;
use crate::{Document, Fault, Node};
pub use buffer::{Buffer, Projection};
use ime::{Effect, Input};
pub use selection::{Direction, Selection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "lowercase",
    deny_unknown_fields
)]
pub enum Edit {
    Insert(String),
    Backward,
    Forward,
    Move { direction: Direction, extend: bool },
    Select(Selection),
    All,
    Commit,
    Activate,
    Blur,
    Next,
}

impl Edit {
    pub fn validate(&self) -> Result<(), Fault> {
        if let Self::Insert(text) = self {
            Buffer::validate(text)?;
        }
        if let Self::Select(selection) = self
            && selection.range().end > 4096
        {
            return Err(Fault("selection exceeds input budget".into()));
        }
        Ok(())
    }

    pub fn kind(&self) -> Option<&'static str> {
        match self {
            Self::Insert(_) => Some("insertText"),
            Self::Backward => Some("deleteContentBackward"),
            Self::Forward => Some("deleteContentForward"),
            _ => None,
        }
    }
}

impl Document {
    pub fn buffer(&self, node: Node) -> Result<&Buffer, Fault> {
        self.entry(node)?;
        self.editors
            .get(&node)
            .ok_or_else(|| Fault("node is not a text input".into()))
    }

    pub fn focused(&self) -> Option<Node> {
        self.focused
    }

    pub fn editable(&self, node: Node) -> bool {
        self.editors.contains_key(&node)
    }

    pub fn enable(&mut self, node: Node, text: Node, caret: Node) -> Result<(), Fault> {
        if self.editors.contains_key(&node) {
            return Err(Fault("node already has an editor".into()));
        }
        if !matches!(self.entry(node)?.kind, crate::Kind::Element(_)) {
            return Err(Fault("editor host must be an element".into()));
        }
        if !matches!(self.entry(text)?.kind, crate::Kind::Text(_))
            || self.entry(text)?.parent != Some(node)
        {
            return Err(Fault("editor text must be a direct text child".into()));
        }
        if !matches!(self.entry(caret)?.kind, crate::Kind::Element(_))
            || self.entry(caret)?.parent != Some(node)
        {
            return Err(Fault("editor caret must be a direct element child".into()));
        }
        self.parts
            .insert(node, crate::document::Parts { text, caret });
        self.editors.insert(node, Buffer::default());
        if self.connected(node)? {
            self.record(Fact::Value { node });
        }
        self.reconcile(node)
    }

    pub fn cursor(&self, node: Node) -> Option<Node> {
        self.parts.get(&node).map(|parts| parts.caret)
    }

    pub fn focusable(&self, node: Node) -> bool {
        matches!(
            self.entry(node).map(|entry| &entry.kind),
            Ok(crate::Kind::Element(tag)) if matches!(tag.as_str(), "button" | "input") || self.editors.contains_key(&node)
        )
    }

    pub fn activatable(&self, node: Node) -> bool {
        matches!(
            self.entry(node).map(|entry| &entry.kind),
            Ok(crate::Kind::Element(tag)) if tag == "button"
        )
    }

    pub fn focus(&mut self, node: Option<Node>) -> Result<(), Fault> {
        if let Some(node) = node {
            if !self.focusable(node) {
                return Err(Fault("node is not focusable".into()));
            }
            if !self.connected(node)? {
                return Err(Fault("cannot focus a detached node".into()));
            }
        }
        if self.focused != node {
            let from = self.focused;
            if let Some(from) = from {
                self.cancel(from);
            }
            self.focused = node;
            self.record(Fact::Focus { from, to: node });
            if let Some(node) = node {
                self.reconcile(node)?;
            }
        }
        Ok(())
    }

    pub fn value(&mut self, node: Node, text: &str) -> Result<(), Fault> {
        let mut buffer = self.buffer(node)?.clone();
        buffer.replace(text)?;
        self.store(node, buffer)
    }

    pub fn caret(&mut self, node: Node, position: usize) -> Result<(), Fault> {
        self.select(node, Selection::caret(position))
    }

    pub fn select(&mut self, node: Node, selection: Selection) -> Result<(), Fault> {
        let mut buffer = self.buffer(node)?.clone();
        buffer.select(selection)?;
        self.store(node, buffer)
    }

    pub fn edit(&mut self, node: Node, edit: &Edit) -> Result<bool, Fault> {
        let mut buffer = self.buffer(node)?.clone();
        let changed = buffer.apply(edit)?;
        self.store(node, buffer)?;
        Ok(changed)
    }

    pub fn compose(&mut self, node: Node, input: &Input) -> Result<Effect, Fault> {
        let mut buffer = self.buffer(node)?.clone();
        let effect = buffer.compose(input)?;
        if effect != Effect::Ignored {
            self.store(node, buffer)?;
        }
        Ok(effect)
    }

    fn cancel(&mut self, node: Node) {
        let changed = self.editors.get_mut(&node).is_some_and(Buffer::cancel);
        if changed && self.connected(node).unwrap_or(false) {
            self.record(Fact::Composition { node });
        }
    }

    fn store(&mut self, node: Node, buffer: Buffer) -> Result<(), Fault> {
        let previous = self.editors.get(&node).expect("validated editor");
        let text = previous.text() != buffer.text();
        let selection = previous.selection() != buffer.selection();
        let composition = previous.composition() != buffer.composition();
        if text || selection || composition {
            let connected = self.connected(node)?;
            self.editors.insert(node, buffer);
            self.reconcile(node)?;
            if text && connected {
                self.record(Fact::Value { node });
            }
            if selection && connected {
                self.record(Fact::Selection { node });
            }
            if composition && connected {
                self.record(Fact::Composition { node });
            }
        }
        Ok(())
    }

    fn reconcile(&mut self, node: Node) -> Result<(), Fault> {
        let Some(parts) = self.parts.get(&node).copied() else {
            return Ok(());
        };
        if self.entry(parts.text)?.parent != Some(node) {
            self.append(node, parts.text)?;
        }
        if self.entry(parts.caret)?.parent != Some(node) {
            self.append(node, parts.caret)?;
        }
        let text = self.buffer(node)?.projection().text;
        self.replace(parts.text, &text)
    }
}
