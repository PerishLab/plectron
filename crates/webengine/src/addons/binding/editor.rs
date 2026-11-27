use super::super::Web;
use crate::{Fault, error};
use domengine::{Node, Selection};
use jsengine::Value;

impl Web {
    pub(super) fn editor(&mut self, arguments: &[Value]) -> Result<Value, Fault> {
        let [host, text, caret, rest @ ..] = arguments else {
            return Err(Fault("invalid Web addon arguments".into()));
        };
        if rest.len() > 1 {
            return Err(Fault("invalid Web addon arguments".into()));
        }
        let multiline = match rest.first() {
            None | Some(Value::Undefined) => false,
            Some(Value::Boolean(flag)) => *flag,
            _ => return Err(Fault("invalid Web addon arguments".into())),
        };
        let host = self.node(host)?;
        let text = self.node(text)?;
        let caret = self.node(caret)?;
        self.document.enable(host, text, caret).map_err(error)?;
        if multiline {
            self.document.multiline(host, true).map_err(error)?;
        }
        Ok(Value::Undefined)
    }

    pub(super) fn caret(&mut self, node: Node, value: &Value) -> Result<Value, Fault> {
        super::super::editing::abandon(self);
        let limit = self.document.buffer(node).map_err(error)?.limit();
        self.document
            .caret(node, position(value, limit)?)
            .map_err(error)?;
        Ok(Value::Undefined)
    }

    pub(super) fn select(
        &mut self,
        node: Node,
        anchor: &Value,
        focus: &Value,
    ) -> Result<Value, Fault> {
        super::super::editing::abandon(self);
        let limit = self.document.buffer(node).map_err(error)?.limit();
        self.document
            .select(
                node,
                Selection {
                    anchor: position(anchor, limit)?,
                    focus: position(focus, limit)?,
                },
            )
            .map_err(error)?;
        Ok(Value::Undefined)
    }
}

fn position(value: &Value, limit: usize) -> Result<usize, Fault> {
    if let Value::Number(position) = value
        && (0.0..=limit as f64).contains(position)
        && position.fract() == 0.0
    {
        return Ok(*position as usize);
    }
    Err(Fault(
        "expected a scalar position within the input budget".into(),
    ))
}
