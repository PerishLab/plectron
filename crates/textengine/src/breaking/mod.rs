mod context;
mod data;
mod rules;
use crate::Fault;
use data::{Class, Property, property};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Soft,
    Hard,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opportunity {
    pub offset: usize,
    pub kind: Kind,
}
struct Unit {
    source: Range<usize>,
    property: Property,
    joined: bool,
    dotted: bool,
    picture: bool,
    spaces: usize,
    number: bool,
    regional: usize,
}
pub fn opportunities(source: &str) -> Result<Vec<Opportunity>, Fault> {
    if source.len() > 65_536 {
        return Err(Fault("line breaking text budget exhausted".into()));
    }
    let units = units(source);
    let mut result = Vec::new();
    for index in 1..units.len() {
        if let Some(kind) = (context::Context {
            units: &units,
            index,
        })
        .boundary()
        {
            result.push(Opportunity {
                offset: units[index].source.start,
                kind,
            });
        }
    }
    result.push(Opportunity {
        offset: source.len(),
        kind: Kind::Hard,
    });
    Ok(result)
}
pub fn mandatory(character: char) -> bool {
    property(character).class.hard()
}
fn units(source: &str) -> Vec<Unit> {
    use Class::*;
    let mut units: Vec<Unit> = Vec::new();
    for (offset, character) in source.char_indices() {
        let mut property = property(character);
        property.class = match property.class {
            Ai | Sg | Xx => Al,
            Cj => Ns,
            Sa if property.mark => Cm,
            Sa => Al,
            class => class,
        };
        let joined = property.class == Zwj;
        if matches!(property.class, Cm | Zwj) {
            if let Some(previous) = units.last_mut().filter(|unit| {
                !unit.property.class.hard() && !matches!(unit.property.class, Sp | Zw)
            }) {
                previous.source.end = offset + character.len_utf8();
                previous.joined = joined;
                continue;
            }
            property = Property::default();
        }
        let previous = units.last();
        let class = property.class;
        units.push(Unit {
            source: offset..offset + character.len_utf8(),
            property,
            joined,
            dotted: character == '\u{25cc}',
            picture: property.unassigned && crate::unicode::picture(character),
            spaces: if class == Sp {
                previous.map_or(1, |unit| unit.spaces + 1)
            } else {
                0
            },
            number: class == Nu
                || (matches!(class, Sy | Is) && previous.is_some_and(|unit| unit.number)),
            regional: if class == Ri {
                previous.map_or(1, |unit| unit.regional + 1)
            } else {
                0
            },
        });
    }
    units
}

pub fn word(character: char) -> bool {
    let property = property(character);
    property.letter
        || matches!(
            property.class,
            Class::Nu | Class::Al | Class::Ai | Class::Id
        )
}
pub fn separator(character: char) -> bool {
    property(character).separator
}
