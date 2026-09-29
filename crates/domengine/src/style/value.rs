use super::{Align, Alignment, Direction, Display, Overflow};
use crate::Fault;
pub(super) struct Value<'a>(pub &'a str);
impl<'a> Value<'a> {
    pub(super) fn intrinsic(self) -> Result<usize, Fault> {
        self.0
            .strip_prefix("auto ")
            .ok_or_else(|| {
                Fault("contain-intrinsic-height requires auto and a pixel length".into())
            })
            .and_then(|value| Value(value).length())
    }

    pub(super) fn length(self) -> Result<usize, Fault> {
        let value = self.0;
        let number = if value == "0" {
            value
        } else {
            value
                .strip_suffix("px")
                .ok_or_else(|| Fault("length requires px".into()))?
        };
        number
            .parse::<usize>()
            .ok()
            .filter(|n| *n <= 4096)
            .ok_or_else(|| Fault("invalid pixel length".into()))
    }

    pub(super) fn color(self) -> Result<u32, Fault> {
        let value = self.0;
        let hex = value
            .strip_prefix('#')
            .filter(|s| s.len() == 6)
            .ok_or_else(|| Fault("color requires #rrggbb".into()))?;
        u32::from_str_radix(hex, 16).map_err(|_| Fault("invalid color".into()))
    }

    pub(super) fn display(self) -> Result<Display, Fault> {
        match self.0 {
            "none" => Ok(Display::None),
            "block" => Ok(Display::Block),
            "inline" => Ok(Display::Inline),
            "flex" => Ok(Display::Flex),
            _ => Err(Fault("unsupported display mode".into())),
        }
    }
    pub(super) fn optional(self, absent: &str) -> Result<Option<usize>, Fault> {
        if self.0 == absent {
            Ok(None)
        } else {
            self.length().map(Some)
        }
    }
    pub(super) fn factor(self) -> Result<f64, Fault> {
        self.0
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && (0.0..=4096.0).contains(value))
            .ok_or_else(|| Fault("invalid flex factor".into()))
    }
    pub(super) fn direction(self) -> Result<Direction, Fault> {
        match self.0 {
            "row" => Ok(Direction::Row),
            "column" => Ok(Direction::Column),
            _ => Err(Fault("only row and column directions are supported".into())),
        }
    }
    pub(super) fn individual(self) -> Result<Option<Alignment>, Fault> {
        if self.0 == "auto" {
            Ok(None)
        } else {
            self.alignment(true).map(Some)
        }
    }
    pub(super) fn alignment(self, cross: bool) -> Result<Alignment, Fault> {
        let (safe, value) = self
            .0
            .strip_prefix("safe ")
            .map_or((false, self.0), |value| (true, value));
        let mode = match value {
            "flex-start" => Align::Start,
            "center" => Align::Center,
            "flex-end" => Align::End,
            "stretch" if cross && !safe => Align::Stretch,
            "space-between" if !cross && !safe => Align::Between,
            _ => return Err(Fault("unsupported alignment".into())),
        };
        Ok(Alignment { mode, safe })
    }

    pub(super) fn translation(self) -> Result<[i64; 2], Fault> {
        let value = self.0;
        let parts: Vec<_> = value.split_whitespace().collect();
        let [x, y] = parts.as_slice() else {
            return Err(Fault("translate requires two pixel lengths".into()));
        };
        Ok([Value(x).signed()?, Value(y).signed()?])
    }
    pub(super) fn signed(self) -> Result<i64, Fault> {
        let value = self.0;
        let (sign, value) = value
            .strip_prefix('-')
            .map_or((1, value), |value| (-1, value));
        Ok(sign * Value(value).length()? as i64)
    }
    pub(super) fn opacity(self) -> Result<u8, Fault> {
        let value = self.0;
        let value: f64 = value.parse().map_err(|_| Fault("invalid opacity".into()))?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Fault("opacity requires 0 through 1".into()));
        }
        Ok((value * 255.0).round() as u8)
    }
    pub(super) fn overflow(self) -> Result<Overflow, Fault> {
        match self.0 {
            "auto" => Ok(Overflow::Auto),
            "hidden" => Ok(Overflow::Hidden),
            _ => Err(Fault("unsupported overflow".into())),
        }
    }
}
