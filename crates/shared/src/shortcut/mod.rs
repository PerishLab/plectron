mod input;
mod scope;
pub use input::{Chord, Input, Modifiers, Stroke};
pub use scope::{Handle, Scope, Set};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Input,
    Handle,
    Budget,
}
impl std::fmt::Display for Fault {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Input => "invalid shortcut input",
            Self::Handle => "foreign or disposed binding set",
            Self::Budget => "shortcut budget exhausted",
        })
    }
}
impl std::error::Error for Fault {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding<C> {
    pub chord: Chord,
    pub command: C,
    pub repeat: bool,
    pub consume: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Outcome {
    pub handled: bool,
    pub consumed: bool,
}

pub fn dispatch<C, E>(
    candidates: impl IntoIterator<Item = Binding<C>>,
    mut execute: impl FnMut(C) -> Result<bool, E>,
) -> Result<Outcome, E> {
    for binding in candidates {
        if execute(binding.command)? {
            return Ok(Outcome {
                handled: true,
                consumed: binding.consume,
            });
        }
    }
    Ok(Outcome::default())
}
