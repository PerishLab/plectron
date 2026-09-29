#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub String);
impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for Fault {}
impl From<std::io::Error> for Fault {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}
impl From<serde_json::Error> for Fault {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}
