use crate::Fault;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Display {
    None,
    Block,
    Inline,
    Flex,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Row,
    Column,
}
impl Direction {
    pub fn axis(self) -> usize {
        usize::from(self == Self::Column)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Between,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Alignment {
    pub mode: Align,
    pub safe: bool,
}
impl Alignment {
    pub fn new(mode: Align) -> Self {
        Self { mode, safe: false }
    }
    pub fn offset(self, free: i64) -> Result<usize, Fault> {
        if free < 0 && !self.safe && matches!(self.mode, Align::Center | Align::End) {
            return Err(Fault(
                "overflowing alignment requires safe alignment in this profile".into(),
            ));
        }
        let free = free.max(0) as usize;
        Ok(match self.mode {
            Align::Center => free / 2,
            Align::End => free,
            _ => 0,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Flex {
    pub direction: Direction,
    pub gap: usize,
    pub justify: Alignment,
    pub align: Alignment,
}
impl Default for Flex {
    fn default() -> Self {
        Self {
            direction: Direction::Row,
            gap: 0,
            justify: Alignment::new(Align::Start),
            align: Alignment::new(Align::Stretch),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Overflow {
    Visible,
    Hidden,
    Auto,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub basis: Option<usize>,
    pub grow: f64,
    pub shrink: f64,
    pub align: Option<Alignment>,
}
impl Default for Item {
    fn default() -> Self {
        Self {
            basis: None,
            grow: 0.0,
            shrink: 1.0,
            align: None,
        }
    }
}
