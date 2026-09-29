#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub String);

impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for Fault {}

impl From<shared::geometry::Fault> for Fault {
    fn from(fault: shared::geometry::Fault) -> Self {
        Self(fault.0)
    }
}
