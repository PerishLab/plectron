#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mode {
    Off,
    Eager,
    Hot(u64),
}

impl Default for Mode {
    fn default() -> Self {
        Self::Hot(32)
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Report {
    pub mode: Mode,
    pub omitted: u64,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Function {
    pub id: usize,
    pub calls: u64,
    pub interpreted: u64,
    pub native: u64,
    pub guards: u64,
    pub budgets: u64,
    pub compilation: u64,
    pub bytes: usize,
    pub rejection: Option<String>,
}
