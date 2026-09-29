mod path;
mod scalar;
pub use path::{Path, Point, Segment, Transform};
pub use scalar::Scalar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub String);
impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for Fault {}
