use crate::style::{Alignment, Display, Flex, Overflow, Style};

#[derive(PartialEq, Eq, Hash)]
pub(in crate::layout) struct Sizing {
    pub isolation: crate::style::Isolation,
    pub typography: crate::style::Typography,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub intrinsic: Option<usize>,
    pub padding: usize,
    pub margin: usize,
    pub display: Display,
    pub flex: Flex,
    pub item: Item,
    pub minimum: [Option<usize>; 2],
    pub maximum: [Option<usize>; 2],
    pub overflow: Overflow,
}

impl From<&Style> for Sizing {
    fn from(style: &Style) -> Self {
        let Style {
            isolation,
            typography,
            width,
            height,
            intrinsic,
            padding,
            margin,
            display,
            flex,
            item,
            minimum,
            maximum,
            overflow,
            color: _,
            background: _,
            translation: _,
            opacity: _,
            animation: _,
        } = style;
        Self {
            isolation: *isolation,
            typography: typography.clone(),
            width: *width,
            height: *height,
            intrinsic: *intrinsic,
            padding: *padding,
            margin: *margin,
            display: *display,
            flex: flex.clone(),
            item: Item {
                basis: item.basis,
                grow: item.grow.to_bits(),
                shrink: item.shrink.to_bits(),
                align: item.align,
            },
            minimum: *minimum,
            maximum: *maximum,
            overflow: *overflow,
        }
    }
}

impl Sizing {
    pub fn independent(&self) -> bool {
        if self.isolation != crate::style::Isolation::None {
            return true;
        }
        if self.overflow != Overflow::Hidden {
            return false;
        }
        self.width.is_some() && self.height.is_some()
    }
    pub fn bound(&self, axis: usize, size: usize) -> usize {
        size.min(self.maximum[axis].unwrap_or(1_048_576))
            .max(self.minimum[axis].unwrap_or(0))
    }
    pub fn preferred(&self, axis: usize) -> Option<usize> {
        [self.width, self.height][axis]
    }
    pub fn inset(&self) -> usize {
        2 * (self.padding + self.margin)
    }
}

#[derive(PartialEq, Eq, Hash)]
pub(in crate::layout) struct Item {
    pub basis: Option<usize>,
    grow: u64,
    shrink: u64,
    pub align: Option<Alignment>,
}
impl Item {
    pub fn grow(&self) -> f64 {
        f64::from_bits(self.grow)
    }
    pub fn shrink(&self) -> f64 {
        f64::from_bits(self.shrink)
    }
}
