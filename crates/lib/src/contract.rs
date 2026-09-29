pub use ::platform::shared::{Extent, Unit, Window};
pub use jsengine::Fault;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    Click,
    Edit(webengine::Edit),
    Input(webengine::keyboard::Input),
    Composition(webengine::ime::Input),
    Close,
    Pointer { x: usize, y: usize },
    Target(webengine::Hit),
    Ready,
    Error(String),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub unit: Unit,
    pub event: Event,
}

impl Event {
    pub fn name(&self) -> &str {
        match self {
            Self::Click | Self::Pointer { .. } | Self::Target(_) => "click",
            Self::Close => "close",
            Self::Edit(_) => "edit",
            Self::Input(_) => "input",
            Self::Composition(_) => "composition",
            Self::Ready => "ready",
            Self::Error(_) => "error",
            Self::Message(_) => "message",
        }
    }
    pub fn input(&self) -> bool {
        matches!(
            self,
            Self::Click
                | Self::Pointer { .. }
                | Self::Target(_)
                | Self::Edit(_)
                | Self::Input(_)
                | Self::Composition(_)
        )
    }
    pub fn data(&self) -> Option<&str> {
        match self {
            Self::Error(text) | Self::Message(text) => Some(text),
            _ => None,
        }
    }
}
