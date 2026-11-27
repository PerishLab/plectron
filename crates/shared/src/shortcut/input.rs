#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Chord {
    pub code: String,
    pub modifiers: Modifiers,
}

impl Chord {
    pub fn parse(source: &str) -> Result<Self, super::Fault> {
        let mut parts = source.split('+').collect::<Vec<_>>();
        let code = parts.pop().ok_or(super::Fault::Input)?;
        let mut modifiers = Modifiers::default();
        for part in parts {
            let flag = match part {
                "Shift" => &mut modifiers.shift,
                "Control" => &mut modifiers.control,
                "Alt" => &mut modifiers.alt,
                "Meta" => &mut modifiers.meta,
                "Primary" if cfg!(target_os = "macos") => &mut modifiers.meta,
                "Primary" => &mut modifiers.control,
                _ => return Err(super::Fault::Input),
            };
            if std::mem::replace(flag, true) {
                return Err(super::Fault::Input);
            }
        }
        let chord = Self {
            code: code.into(),
            modifiers,
        };
        chord.validate()?;
        Ok(chord)
    }

    pub fn validate(&self) -> Result<(), super::Fault> {
        let letter = self
            .code
            .strip_prefix("Key")
            .is_some_and(|code| code.len() == 1 && code.as_bytes()[0].is_ascii_uppercase());
        if letter
            || matches!(
                self.code.as_str(),
                "ArrowLeft"
                    | "ArrowRight"
                    | "ArrowUp"
                    | "ArrowDown"
                    | "Home"
                    | "End"
                    | "Backspace"
                    | "Delete"
                    | "Enter"
                    | "Escape"
                    | "Tab"
                    | "Space"
            )
        {
            Ok(())
        } else {
            Err(super::Fault::Input)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Stroke {
    pub chord: Chord,
    pub repeat: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "type",
        content = "data",
        rename_all = "lowercase",
        deny_unknown_fields
    )
)]
pub enum Input {
    Key(Stroke),
    Text(String),
}
impl Input {
    pub fn validate(&self) -> Result<(), super::Fault> {
        self.check(false)
    }

    pub fn multiline(&self) -> Result<(), super::Fault> {
        self.check(true)
    }

    fn check(&self, multiline: bool) -> Result<(), super::Fault> {
        let text = match self {
            Self::Key(stroke) => {
                stroke.chord.validate()?;
                &stroke.text
            }
            Self::Text(text) => text,
        };
        let limit = if multiline { 16_384 } else { 4096 };
        let invalid = text.chars().any(|character| {
            character.is_control() && !(multiline && matches!(character, '\n' | '\r'))
        });
        if text.len() > limit || invalid {
            Err(super::Fault::Input)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chord, Input, Stroke};

    #[test]
    fn validation() {
        let single = Input::Text("a\nb".into());
        assert!(single.validate().is_err());
        assert!(single.multiline().is_ok());
        let crlf = Input::Text("a\r\nb".into());
        assert!(crlf.validate().is_err());
        assert!(crlf.multiline().is_ok());
        let control = Input::Text("a\0b".into());
        assert!(control.validate().is_err());
        assert!(control.multiline().is_err());
        let long = Input::Text("a".repeat(8000));
        assert!(long.validate().is_err());
        assert!(long.multiline().is_ok());
        let chord = Stroke {
            chord: Chord::parse("Shift+Enter").unwrap(),
            repeat: false,
            text: String::new(),
        };
        assert!(Input::Key(chord).validate().is_ok());
    }
}
