mod properties;
use properties::{Property, Tables};
use std::ops::Range;

pub fn graphemes(source: &str) -> Vec<Range<usize>> {
    let tables = Tables::get();
    let mut state = State::default();
    let mut ranges = Vec::new();
    let mut start = 0;
    for (offset, character) in source.char_indices() {
        let properties = tables.properties(character);
        if offset > 0 && state.boundary(properties) {
            ranges.push(start..offset);
            start = offset;
        }
        state.advance(properties);
    }
    if !source.is_empty() {
        ranges.push(start..source.len());
    }
    ranges
}

#[derive(Default)]
struct State {
    previous: Property,
    regional: usize,
    indic: u8,
    picture: bool,
    joined: bool,
}
impl State {
    fn boundary(&self, current: (Property, Property, bool)) -> bool {
        use Property::*;
        let (next, indic, picture) = current;
        if (self.previous, next) == (Cr, Lf) {
            return false;
        }
        if matches!(self.previous, Cr | Lf | Control) || matches!(next, Cr | Lf | Control) {
            return true;
        }
        if hangul(self.previous, next)
            || matches!(next, Extend | Zwj | Spacing)
            || self.previous == Prepend
        {
            return false;
        }
        if indic == Consonant && self.indic == 2 {
            return false;
        }
        if picture && self.previous == Zwj && self.joined {
            return false;
        }
        !(next == Regional && self.regional % 2 == 1)
    }
    fn advance(&mut self, current: (Property, Property, bool)) {
        use Property::*;
        let (next, indic, picture) = current;
        self.regional = if next == Regional {
            self.regional + 1
        } else {
            0
        };
        self.indic = match indic {
            Consonant => 1,
            Linker if self.indic > 0 => 2,
            Extend => self.indic,
            _ => 0,
        };
        self.joined = next == Zwj && self.picture;
        self.picture = picture || (next == Extend && self.picture);
        self.previous = next;
    }
}
fn hangul(previous: Property, next: Property) -> bool {
    use Property::*;
    match previous {
        L => matches!(next, L | V | Lv | Lvt),
        Lv | V => matches!(next, V | T),
        Lvt | T => next == T,
        _ => false,
    }
}

mod joining;
pub(crate) use joining::forms;
pub use joining::{connections, hidden};

mod combining;
pub(crate) use combining::{combining, order};

pub(crate) fn picture(character: char) -> bool {
    Tables::get().picture(character)
}
