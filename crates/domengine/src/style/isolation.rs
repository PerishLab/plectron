use crate::Fault;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Isolation {
    #[default]
    None,
    Box,
}
impl Isolation {
    pub(super) fn parse(value: &str) -> Result<Self, Fault> {
        if value == "none" {
            return Ok(Self::None);
        }
        let mut parts: Vec<_> = value.split_whitespace().collect();
        parts.sort_unstable();
        if parts == ["layout", "paint", "size"] {
            Ok(Self::Box)
        } else {
            Err(Fault("contain supports none or size layout paint".into()))
        }
    }
}
