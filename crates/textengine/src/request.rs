use crate::{Fault, Scalar, Weight};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Request {
    pub families: Arc<[String]>,
    pub size: Scalar,
    pub weight: Weight,
}
impl Default for Request {
    fn default() -> Self {
        static FAMILIES: OnceLock<Arc<[String]>> = OnceLock::new();
        Self {
            families: FAMILIES.get_or_init(|| Arc::from(["toy".into()])).clone(),
            size: 16.into(),
            weight: Weight::Normal,
        }
    }
}
impl Request {
    pub fn retained(&self) -> usize {
        self.families
            .iter()
            .map(|name| std::mem::size_of::<String>() + name.len())
            .sum()
    }
    pub fn validate(&self) -> Result<(), Fault> {
        if self.families.is_empty() || self.families.len() > 32 {
            return Err(Fault("font candidate budget exhausted".into()));
        }
        if self.size < Scalar::ZERO || self.size > 4096.into() {
            return Err(Fault("font size budget exhausted".into()));
        }
        for family in self.families.iter() {
            family::validate(family)?;
        }
        Ok(())
    }
}
pub(crate) mod family {
    use crate::Fault;
    pub fn validate(name: &str) -> Result<(), Fault> {
        if name.is_empty() || name.len() > 128 || name.trim() != name {
            return Err(Fault("invalid font family".into()));
        }
        if !name
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_' | ' '))
        {
            return Err(Fault("unsupported font family spelling".into()));
        }
        Ok(())
    }
}
