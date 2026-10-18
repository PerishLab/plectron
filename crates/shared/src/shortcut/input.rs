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
        let text = match self {
            Self::Key(stroke) => {
                stroke.chord.validate()?;
                &stroke.text
            }
            Self::Text(text) => text,
        };
        if text.len() > 4096 || text.chars().any(char::is_control) {
            Err(super::Fault::Input)
        } else {
            Ok(())
        }
    }
}
