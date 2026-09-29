use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub String);

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl std::error::Error for Fault {}

#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Throw(crate::Value),
    Fault(Fault),
}

impl From<Fault> for Failure {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Throw(value) => write!(formatter, "uncaught exception: {}", value.text()),
            Self::Fault(fault) => fault.fmt(formatter),
        }
    }
}

impl std::error::Error for Failure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Syntax(Fault),
    Fault(Fault),
}

impl Rejection {
    pub(crate) fn syntax(message: impl Into<String>) -> Self {
        Self::Syntax(Fault(message.into()))
    }
}

impl From<Fault> for Rejection {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(fault) | Self::Fault(fault) => fault.fmt(formatter),
        }
    }
}

impl std::error::Error for Rejection {}
