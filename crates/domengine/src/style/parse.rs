use super::{Fault, Style, apply};
use grammar::{Cursor, Grammar};
use std::collections::BTreeMap;

type Properties = BTreeMap<String, String>;

pub(super) fn rules(source: &str) -> Result<super::Sheet, Fault> {
    let mut reader = Reader::new(source)?;
    let mut sheet = super::Sheet::empty();
    reader.eat("space")?;
    while !reader.eat("end")? {
        if reader.eat("keyframes")? {
            sheet.insert(reader.keyframes()?);
            reader.eat("space")?;
            continue;
        }
        let font = reader.eat("font")?;
        let selector = if font {
            ""
        } else {
            reader.require("selector", "only tag, id and class selectors are supported")?
        };
        reader.eat("space")?;
        reader.require("open", "expected CSS rule")?;
        let properties = reader.properties(true)?;
        reader.require("close", "unclosed CSS rule")?;
        if font {
            sheet.fonts.push(super::Font::parse(properties)?);
        } else {
            apply(&mut Style::default(), &properties).map_err(|error| reader.context(error))?;
            sheet.rules.push((selector.into(), properties));
        }
        if sheet.rules.len() + sheet.fonts.len() + sheet.keyframes.len() > 256 {
            return Err(Fault("CSS rule budget exhausted".into()));
        }
        reader.eat("space")?;
    }
    Ok(sheet)
}

pub fn declarations(source: &str) -> Result<Properties, Fault> {
    let properties = Reader::new(source)?.properties(false)?;
    apply(&mut Style::default(), &properties)?;
    Ok(properties)
}

struct Reader<'a> {
    cursor: Cursor<'a>,
    grammar: &'static Grammar,
}

impl<'a> Reader<'a> {
    fn keyframes(&mut self) -> Result<super::Keyframes, Fault> {
        self.eat("space")?;
        let name = self
            .require("selector", "expected keyframes name")?
            .to_owned();
        self.eat("space")?;
        self.require("open", "expected keyframes block")?;
        let mut frames = Vec::new();
        loop {
            self.eat("space")?;
            if self.eat("close")? {
                break;
            }
            let selector = self.require("keyframe", "expected keyframe selector")?;
            let offset = super::animation::Parse::offset(selector)?;
            self.eat("space")?;
            self.require("open", "expected keyframe block")?;
            let properties = self.properties(true)?;
            self.require("close", "unclosed keyframe block")?;
            let value = properties
                .get("opacity")
                .ok_or_else(|| self.context(Fault("keyframe requires opacity".into())))?;
            frames.push((offset, super::Value(value).opacity()?));
        }
        super::Keyframes::new(name, frames)
    }
    fn new(source: &'a str) -> Result<Self, Fault> {
        let grammar = crate::resource::css()?;
        Ok(Self {
            cursor: Cursor::new(source).map_err(fault)?,
            grammar,
        })
    }

    fn properties(&mut self, block: bool) -> Result<Properties, Fault> {
        let mut properties = BTreeMap::new();
        loop {
            self.eat("space")?;
            if self.eat("end")? || (block && self.eat("boundary")?) {
                break;
            }
            if self.eat("semicolon")? {
                continue;
            }
            let name = self.require("property", "invalid CSS declaration")?.trim();
            self.require("colon", "invalid CSS declaration")?;
            let value = self.require("value", "invalid CSS value")?.trim();
            properties.insert(name.into(), value.into());
        }
        Ok(properties)
    }

    fn eat(&mut self, rule: &str) -> Result<bool, Fault> {
        Ok(self
            .grammar
            .matches(rule, &mut self.cursor)
            .map_err(fault)?
            .is_some())
    }

    fn require(&mut self, rule: &str, note: &str) -> Result<&'a str, Fault> {
        self.grammar
            .matches(rule, &mut self.cursor)
            .map_err(fault)?
            .map(|matched| matched.text)
            .ok_or_else(|| self.context(Fault(note.into())))
    }

    fn context(&self, error: Fault) -> Fault {
        Fault(format!("CSS at byte {}: {error}", self.cursor.offset()))
    }
}

fn fault(error: impl std::fmt::Display) -> Fault {
    Fault(error.to_string())
}
