use super::{Display, Overflow, Style, Typography, apply};
use crate::Fault;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Inherited {
    pub color: u32,
    pub typography: Typography,
}
impl Default for Inherited {
    fn default() -> Self {
        Self::from(&Style::default())
    }
}
impl From<&Style> for Inherited {
    fn from(style: &Style) -> Self {
        Self {
            color: style.color,
            typography: style.typography.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Specified {
    pub(super) tag: Option<String>,
    pub(super) direction: Option<textengine::bidi::Direction>,
    pub(super) properties: BTreeMap<String, String>,
}
impl Specified {
    pub fn compute(&self, inherited: Inherited) -> Result<Style, Fault> {
        let mut style = Style {
            color: inherited.color,
            typography: inherited.typography,
            ..Style::default()
        };
        if let Some(direction) = self.direction {
            style.typography.direction = direction;
        }
        if matches!(
            self.tag.as_deref(),
            Some("head" | "title" | "link" | "script" | "style")
        ) {
            style.display = Display::None;
        }
        if matches!(self.tag.as_deref(), Some("span" | "strong" | "a")) {
            style.display = Display::Inline;
        }
        if self.tag.as_deref() == Some("strong") {
            style.typography.font.weight = textengine::Weight::Bold;
        }
        if self.tag.as_deref() == Some("input") {
            style.overflow = Overflow::Hidden;
        }
        apply(&mut style, &self.properties)?;
        Ok(style)
    }
    pub(super) fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.tag.as_ref().map_or(0, String::len)
            + self
                .properties
                .iter()
                .map(|(key, value)| key.len() + value.len() + 128)
                .sum::<usize>()
    }
}
