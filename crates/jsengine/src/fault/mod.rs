use std::fmt;

mod execution;
mod source;
pub use source::{Site, Source, Weak};

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
    Located(Box<Failure>, Site),
}

impl Failure {
    pub fn cause(&self) -> &Self {
        match self {
            Self::Located(failure, _) => failure.cause(),
            failure => failure,
        }
    }

    pub fn site(&self) -> Option<&Site> {
        match self {
            Self::Located(_, site) => Some(site),
            _ => None,
        }
    }

    pub fn thrown(&self) -> Option<&crate::Value> {
        match self.cause() {
            Self::Throw(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn at(self, site: Option<Site>) -> Self {
        if self.site().is_some() {
            return self;
        }
        match site {
            Some(site) => Self::Located(Box::new(self), site),
            None => self,
        }
    }
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
            Self::Located(failure, _) => failure.fmt(formatter),
        }
    }
}

impl std::error::Error for Failure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Syntax(Syntax),
    Fault(Fault),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syntax {
    pub message: String,
    pub span: Option<std::ops::Range<usize>>,
}

impl fmt::Display for Syntax {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.message)
    }
}

impl std::error::Error for Syntax {}

impl Rejection {
    pub(crate) fn syntax(message: impl Into<String>) -> Self {
        Self::Syntax(Syntax {
            message: message.into(),
            span: None,
        })
    }

    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        match self {
            Self::Syntax(syntax) => syntax.span.clone(),
            Self::Fault(_) => None,
        }
    }

    pub(crate) fn at(mut self, span: std::ops::Range<usize>) -> Self {
        if let Self::Syntax(syntax) = &mut self {
            syntax.span.get_or_insert(span);
        }
        self
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
            Self::Syntax(syntax) => syntax.fmt(formatter),
            Self::Fault(fault) => fault.fmt(formatter),
        }
    }
}

impl std::error::Error for Rejection {}
