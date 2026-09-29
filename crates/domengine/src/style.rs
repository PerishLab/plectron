use crate::{Document, Fault, Node};
use std::collections::BTreeMap;

pub mod animation;
mod flex;
pub use animation::{Animation, Keyframes, Timing};
mod isolation;
pub use isolation::Isolation;
#[path = "style/typography/font.rs"]
mod font;
pub use font::Font;
mod resolver;
mod specified;
mod subject;
pub use resolver::Resolver;
pub use specified::{Inherited, Specified};
pub use subject::Subject;
mod parse;
pub mod typography;
mod value;
pub use flex::{Align, Alignment, Direction, Display, Flex, Item, Overflow};
pub use parse::declarations;
pub use typography::Typography;
use value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub isolation: Isolation,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub intrinsic: Option<usize>,
    pub padding: usize,
    pub margin: usize,
    pub color: u32,
    pub typography: Typography,
    pub background: Option<u32>,
    pub display: Display,
    pub flex: Flex,
    pub item: Item,
    pub minimum: [Option<usize>; 2],
    pub maximum: [Option<usize>; 2],
    pub translation: [i64; 2],
    pub opacity: u8,
    pub animation: Animation,
    pub overflow: Overflow,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            isolation: Isolation::None,
            width: None,
            height: None,
            intrinsic: None,
            padding: 0,
            margin: 0,
            color: 0x182330,
            typography: Typography::default(),
            background: None,
            display: Display::Block,
            flex: Flex::default(),
            item: Item::default(),
            minimum: [None; 2],
            maximum: [None; 2],
            translation: [0, 0],
            opacity: 255,
            animation: Animation::default(),
            overflow: Overflow::Visible,
        }
    }
}

#[derive(Clone)]
pub struct Sheet {
    rules: Vec<(String, BTreeMap<String, String>)>,
    fonts: Vec<Font>,
    keyframes: Vec<Keyframes>,
    revision: u64,
    sequence: u64,
}

impl Default for Sheet {
    fn default() -> Self {
        Self::empty()
    }
}

impl Sheet {
    pub(super) fn empty() -> Self {
        Self {
            rules: Vec::new(),
            fonts: Vec::new(),
            keyframes: Vec::new(),
            revision: 0,
            sequence: 1,
        }
    }
    pub fn parse(source: &str) -> Result<Self, Fault> {
        parse::rules(source)
    }

    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }
    pub fn keyframes(&self, name: &str) -> Option<&Keyframes> {
        self.keyframes.iter().rev().find(|rule| rule.name == name)
    }
    pub fn rule(&self, id: u64) -> Option<&Keyframes> {
        self.keyframes.iter().find(|rule| rule.id == id)
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn set(&mut self, id: u64, offset: f64, opacity: u8) -> Result<(), Fault> {
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(Fault("invalid keyframe offset".into()));
        }
        let rule = self
            .keyframes
            .iter_mut()
            .find(|rule| rule.id == id)
            .ok_or_else(|| Fault("unknown keyframes rule".into()))?;
        if let Some((_, value)) = rule.opacity.iter_mut().find(|(point, _)| *point == offset) {
            if *value == opacity {
                return Ok(());
            }
            *value = opacity;
        } else {
            if rule.opacity.len() >= 64 {
                return Err(Fault("keyframe count exceeds budget".into()));
            }
            rule.opacity.push((offset, opacity));
            rule.opacity.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        rule.revision = rule.revision.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub(super) fn insert(&mut self, mut rule: Keyframes) {
        rule.id = self.sequence;
        self.sequence += 1;
        self.keyframes.push(rule);
    }
    pub fn extend(&mut self, sheet: Self) -> Result<(), Fault> {
        if self.rules.len()
            + self.fonts.len()
            + self.keyframes.len()
            + sheet.rules.len()
            + sheet.fonts.len()
            + sheet.keyframes.len()
            > 256
        {
            return Err(Fault("CSS rule budget exhausted".into()));
        }
        self.rules.extend(sheet.rules);
        self.fonts.extend(sheet.fonts);
        for rule in sheet.keyframes {
            self.insert(rule);
        }
        Ok(())
    }
    pub fn compute(
        &self,
        document: &Document,
        node: Node,
        inherited: Inherited,
    ) -> Result<Style, Fault> {
        self.matching(&Subject::capture(document, node)?)?
            .compute(inherited)
    }
    pub fn matching(&self, subject: &Subject) -> Result<Specified, Fault> {
        let mut properties = BTreeMap::new();
        let mut matched: Vec<_> = self
            .rules
            .iter()
            .filter(|(selector, _)| subject.matches(selector))
            .collect();
        matched.sort_by_key(|(selector, _)| specificity(selector));
        for (_, declarations) in matched {
            properties.extend(declarations.clone());
        }
        if subject.tag.is_some()
            && let Some(inline) = &subject.inline
        {
            properties.extend(declarations(inline)?);
        }
        Ok(Specified {
            tag: subject.tag.clone(),
            direction: subject
                .direction
                .as_deref()
                .map(|value| match value {
                    "ltr" => Ok(textengine::bidi::Direction::Ltr),
                    "rtl" => Ok(textengine::bidi::Direction::Rtl),
                    "auto" => Ok(textengine::bidi::Direction::Auto),
                    _ => Err(Fault("dir requires ltr, rtl or auto".into())),
                })
                .transpose()?,
            properties,
        })
    }
}

fn specificity(selector: &str) -> u8 {
    match selector.as_bytes()[0] {
        b'#' => 2,
        b'.' => 1,
        _ => 0,
    }
}

fn apply(style: &mut Style, properties: &BTreeMap<String, String>) -> Result<(), Fault> {
    for (name, value) in properties {
        let value = Value(value);
        match name.as_str() {
            "contain" => style.isolation = Isolation::parse(value.0)?,
            "width" => style.width = value.optional("auto")?,
            "height" => style.height = value.optional("auto")?,
            "contain-intrinsic-height" => style.intrinsic = Some(value.intrinsic()?),
            "padding" => style.padding = value.length()?,
            "margin" => style.margin = value.length()?,
            "color" => style.color = value.color()?,
            "background-color" => style.background = Some(value.color()?),
            "translate" => style.translation = value.translation()?,
            "opacity" => style.opacity = value.opacity()?,
            "animation-name" => style.animation.name = animation::Parse::name(value.0)?,
            "animation-duration" => style.animation.duration = animation::Parse::duration(value.0)?,
            "animation-delay" => style.animation.delay = animation::Parse::delay(value.0)?,
            "animation-iteration-count" => {
                style.animation.iterations = animation::Parse::iterations(value.0)?
            }
            "animation-timing-function" => style.animation.timing = Timing::parse(value.0)?,
            "animation-play-state" => style.animation.playing = animation::Parse::playing(value.0)?,
            "overflow" => style.overflow = value.overflow()?,
            "display" => style.display = value.display()?,
            "flex-direction" => style.flex.direction = value.direction()?,
            "flex-wrap" if value.0 == "nowrap" => (),
            "gap" => style.flex.gap = value.length()?,
            "justify-content" => style.flex.justify = value.alignment(false)?,
            "align-items" => style.flex.align = value.alignment(true)?,
            "align-self" => style.item.align = value.individual()?,
            "flex-basis" => style.item.basis = value.optional("auto")?,
            "flex-grow" => style.item.grow = value.factor()?,
            "flex-shrink" => style.item.shrink = value.factor()?,
            "min-width" => style.minimum[0] = value.optional("auto")?,
            "min-height" => style.minimum[1] = value.optional("auto")?,
            "max-width" => style.maximum[0] = value.optional("none")?,
            "max-height" => style.maximum[1] = value.optional("none")?,
            _ => style.typography.apply(name, value)?,
        }
    }
    Ok(())
}
