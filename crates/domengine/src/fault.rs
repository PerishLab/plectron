#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub String);

impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for Fault {}

impl From<graphics::shared::Fault> for Fault {
    fn from(fault: graphics::shared::Fault) -> Self {
        Self(fault.0)
    }
}

impl From<textengine::Fault> for Fault {
    fn from(fault: textengine::Fault) -> Self {
        Self(fault.0)
    }
}
