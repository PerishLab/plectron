use crate::Fault;
use std::borrow::Cow;

pub(super) const SINGLE: usize = 4096;
pub(super) const MULTI: usize = 16_384;

#[derive(Clone, Copy)]
pub(super) struct Policy(pub bool);

impl Policy {
    pub(super) fn limit(self) -> usize {
        if self.0 { MULTI } else { SINGLE }
    }

    pub(super) fn prepare<'a>(self, value: &'a str) -> Cow<'a, str> {
        if self.0 && value.contains('\r') {
            Cow::Owned(value.replace("\r\n", "\n").replace('\r', "\n"))
        } else {
            Cow::Borrowed(value)
        }
    }

    pub(super) fn validate(self, value: &str) -> Result<(), Fault> {
        let normalized = self.prepare(value);
        let invalid = normalized
            .chars()
            .any(|character| character.is_control() && !(self.0 && character == '\n'));
        if normalized.len() > self.limit() || invalid {
            let message = if self.0 {
                "input requires at most 16384 bytes of multiline text"
            } else {
                "input requires at most 4096 bytes of single-line text"
            };
            return Err(Fault(message.into()));
        }
        Ok(())
    }

    pub(super) fn sanitize(self, value: &str) -> String {
        let normalized = Policy(true).prepare(value);
        normalized
            .chars()
            .filter(|character| !character.is_control() || (self.0 && *character == '\n'))
            .collect()
    }
}

pub(super) fn byte(text: &str, position: usize) -> usize {
    text.char_indices()
        .nth(position)
        .map_or(text.len(), |(byte, _)| byte)
}
