mod data;
mod neutral;
mod order;
mod weak;
use crate::Fault;
pub use data::{Class, class, mirror};
pub use order::reorder;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
    Auto,
}
#[derive(Debug, Clone)]
pub struct Unit {
    pub source: Range<usize>,
    pub level: u8,
    pub class: Class,
    character: char,
}
#[derive(Debug, Clone)]
pub struct Paragraph {
    pub source: Range<usize>,
    pub level: u8,
}
#[derive(Debug, Clone)]
pub struct Bidi {
    pub units: Vec<Unit>,
    pub paragraphs: Vec<Paragraph>,
}
impl Bidi {
    pub fn resolve(source: &str, direction: Direction) -> Result<Self, Fault> {
        if source.len() > 65_536 {
            return Err(Fault("bidi text budget exhausted".into()));
        }
        let mut bidi = Self {
            units: source
                .char_indices()
                .map(|(start, character)| Unit {
                    source: start..start + character.len_utf8(),
                    level: 0,
                    class: class(character),
                    character,
                })
                .collect(),
            paragraphs: Vec::new(),
        };
        if direction != Direction::Rtl
            && bidi
                .units
                .iter()
                .all(|unit| matches!(unit.class, Class::L | Class::En | Class::Ws))
        {
            for unit in &mut bidi.units {
                unit.level = if unit.class == Class::En { 2 } else { 0 };
            }
            bidi.paragraphs.push(Paragraph {
                source: 0..source.len(),
                level: 0,
            });
            return Ok(bidi);
        }
        if bidi.units.iter().any(|unit| unit.class == Class::Legacy) {
            return Err(Fault(
                "legacy bidi embeddings and overrides are excluded; use isolates".into(),
            ));
        }
        if bidi.units.is_empty() {
            bidi.paragraphs.push(Paragraph {
                source: 0..0,
                level: u8::from(direction == Direction::Rtl),
            });
        }
        let mut start = 0;
        while start < bidi.units.len() {
            let end = bidi.units[start..]
                .iter()
                .position(|unit| unit.class == Class::B)
                .map_or(bidi.units.len(), |index| start + index + 1);
            let level = base(&bidi.units[start..end], direction);
            bidi.paragraphs.push(Paragraph {
                source: bidi.units[start].source.start..bidi.units[end - 1].source.end,
                level,
            });
            bidi.sequence(start..end, level)?;
            start = end;
        }
        Ok(bidi)
    }
    fn sequence(&mut self, range: Range<usize>, level: u8) -> Result<(), Fault> {
        if level > 125 {
            return Err(Fault("bidi isolate depth budget exhausted".into()));
        }
        let mut indices = Vec::new();
        let mut index = range.start;
        while index < range.end {
            self.units[index].level = level;
            let class = self.units[index].class;
            if class != Class::Bn {
                indices.push(index);
            }
            if class.isolate() {
                let end = closing(&self.units, index + 1..range.end);
                let direction = match class {
                    Class::Lri => Direction::Ltr,
                    Class::Rli => Direction::Rtl,
                    _ => Direction::Auto,
                };
                let parity = base(&self.units[index + 1..end], direction);
                let nested = level + 1 + u8::from((level + 1) % 2 != parity);
                self.sequence(index + 1..end, nested)?;
                index = end;
            } else {
                index += 1;
            }
        }
        let mut types: Vec<_> = indices
            .iter()
            .map(|index| self.units[*index].class)
            .collect();
        let boundary = if level.is_multiple_of(2) {
            Class::L
        } else {
            Class::R
        };
        weak::resolve(&mut types, boundary);
        neutral::resolve(&self.units, &indices, &mut types, boundary);
        for (index, class) in indices.into_iter().zip(types) {
            self.units[index].level = level
                + if level.is_multiple_of(2) {
                    match class {
                        Class::R => 1,
                        Class::En | Class::An => 2,
                        _ => 0,
                    }
                } else {
                    u8::from(class != Class::R)
                };
        }
        Ok(())
    }
}
fn base(units: &[Unit], direction: Direction) -> u8 {
    match direction {
        Direction::Ltr => return 0,
        Direction::Rtl => return 1,
        Direction::Auto => (),
    }
    let mut depth: usize = 0;
    for unit in units {
        if unit.class.isolate() {
            depth += 1;
        } else if unit.class == Class::Pdi {
            depth = depth.saturating_sub(1);
        } else if depth == 0 {
            match unit.class {
                Class::L => return 0,
                Class::R | Class::Al => return 1,
                _ => (),
            }
        }
    }
    0
}
fn closing(units: &[Unit], range: Range<usize>) -> usize {
    let mut depth: usize = 0;
    for index in range.clone() {
        if units[index].class.isolate() {
            depth += 1;
        } else if units[index].class == Class::Pdi {
            if depth == 0 {
                return index;
            }
            depth -= 1;
        }
    }
    range.end
}
