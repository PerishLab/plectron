use super::value::Value;
use crate::Fault;
use shared::geometry::Scalar;
use textengine::{Request, Weight};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Leading {
    Normal,
    Factor(u32),
    Pixels(u16),
}
impl Leading {
    pub fn height(self, size: Scalar) -> usize {
        match self {
            Self::Normal => size.ceil(),
            Self::Factor(value) => (size.value() * f64::from(value) / 1000.0).round() as usize,
            Self::Pixels(value) => usize::from(value),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    Start,
    Center,
    End,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Whitespace {
    Normal,
    Nowrap,
    Pre,
    Prewrap,
    Spaces,
}
impl Whitespace {
    pub fn collapse(self) -> bool {
        matches!(self, Self::Normal | Self::Nowrap)
    }
    pub fn wrap(self) -> bool {
        matches!(self, Self::Normal | Self::Prewrap | Self::Spaces)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Wrapping {
    Normal,
    Anywhere,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Words {
    Normal,
    Keep,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Typography {
    pub words: Words,
    pub font: Request,
    pub direction: textengine::bidi::Direction,
    pub leading: Leading,
    pub spacing: usize,
    pub align: Align,
    pub whitespace: Whitespace,
    pub wrapping: Wrapping,
}
impl Default for Typography {
    fn default() -> Self {
        Self {
            font: Request::default(),
            words: Words::Normal,
            direction: textengine::bidi::Direction::Ltr,
            leading: Leading::Normal,
            spacing: 0,
            align: Align::Start,
            whitespace: Whitespace::Normal,
            wrapping: Wrapping::Normal,
        }
    }
}
impl Typography {
    pub(super) fn apply(&mut self, name: &str, value: Value<'_>) -> Result<(), Fault> {
        match name {
            "word-break" => {
                self.words = match value.0 {
                    "normal" => Words::Normal,
                    "keep-all" => Words::Keep,
                    _ => return Err(Fault("word-break requires normal or keep-all".into())),
                }
            }
            "direction" => {
                self.direction = match value.0 {
                    "ltr" => textengine::bidi::Direction::Ltr,
                    "rtl" => textengine::bidi::Direction::Rtl,
                    _ => return Err(Fault("direction requires ltr or rtl".into())),
                }
            }
            "font-family" => {
                self.font.families = value.0.split(',').map(family).collect();
                self.font.validate()?;
            }
            "font-size" => self.font.size = value.size()?,
            "font-weight" => {
                self.font.weight = match value.0 {
                    "normal" | "400" => Weight::Normal,
                    "bold" | "700" => Weight::Bold,
                    _ => return Err(Fault("font-weight requires normal/400 or bold/700".into())),
                }
            }
            "line-height" => self.leading = value.leading()?,
            "letter-spacing" => self.spacing = value.optional("normal")?.unwrap_or(0),
            "text-align" => {
                self.align = match value.0 {
                    "start" => Align::Start,
                    "center" => Align::Center,
                    "end" => Align::End,
                    _ => return Err(Fault("text-align requires start, center or end".into())),
                }
            }
            "white-space" => {
                self.whitespace = match value.0 {
                    "normal" => Whitespace::Normal,
                    "nowrap" => Whitespace::Nowrap,
                    "pre" => Whitespace::Pre,
                    "pre-wrap" => Whitespace::Prewrap,
                    "break-spaces" => Whitespace::Spaces,
                    _ => return Err(Fault("unsupported white-space mode".into())),
                }
            }
            "overflow-wrap" => {
                self.wrapping = match value.0 {
                    "normal" => Wrapping::Normal,
                    "anywhere" => Wrapping::Anywhere,
                    _ => return Err(Fault("overflow-wrap requires normal or anywhere".into())),
                }
            }
            _ => return Err(Fault(format!("unsupported CSS property: {name}"))),
        }
        Ok(())
    }
}
impl Value<'_> {
    fn size(self) -> Result<Scalar, Fault> {
        let number = if self.0 == "0" {
            self.0
        } else {
            self.0
                .strip_suffix("px")
                .ok_or_else(|| Fault("font-size requires px".into()))?
        };
        Ok(Scalar::new(Value(number).factor()?))
    }

    fn leading(self) -> Result<Leading, Fault> {
        if self.0 == "normal" {
            return Ok(Leading::Normal);
        }
        if self.0.ends_with("px") {
            return Ok(Leading::Pixels(self.length()? as u16));
        }
        let factor = self
            .0
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && (0.0..=16.0).contains(value))
            .ok_or_else(|| {
                Fault("line-height requires normal, pixels or a factor up to 16".into())
            })?;
        Ok(Leading::Factor((factor * 1000.0).round() as u32))
    }
}

fn family(name: &str) -> String {
    let name = name.trim();
    name.strip_prefix('"')
        .and_then(|name| name.strip_suffix('"'))
        .or_else(|| {
            name.strip_prefix('\'')
                .and_then(|name| name.strip_suffix('\''))
        })
        .unwrap_or(name)
        .to_lowercase()
}

impl Align {
    pub(crate) fn offset(self, remaining: usize, reversed: bool) -> usize {
        match (self, reversed) {
            (Self::Start, false) | (Self::End, true) => 0,
            (Self::Center, _) => remaining / 2,
            _ => remaining,
        }
    }
}
