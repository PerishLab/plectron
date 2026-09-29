use super::coverage::Coverage;
use crate::font::{Problem, data::Data};

#[derive(Clone)]
pub(super) struct Filter<'a> {
    pub flags: u16,
    definition: Option<Data<'a>>,
    marks: Option<Coverage<'a>>,
}
pub(super) struct Record {
    flags: u16,
    definition: Option<std::ops::Range<usize>>,
    marks: Option<super::coverage::Record>,
}
impl Record {
    pub fn view<'a>(&self, source: &'a [u8]) -> Filter<'a> {
        Filter {
            flags: self.flags,
            definition: self
                .definition
                .as_ref()
                .map(|range| Data(&source[range.clone()])),
            marks: self.marks.as_ref().map(|marks| marks.view(source)),
        }
    }
}
impl<'a> Filter<'a> {
    pub fn record(&self, source: &[u8]) -> Record {
        Record {
            flags: self.flags,
            definition: self.definition.map(|data| data.range(source)),
            marks: self.marks.as_ref().map(|marks| marks.record(source)),
        }
    }
    pub fn new(data: Data<'a>, definition: Option<Data<'a>>) -> Result<Self, Problem> {
        let flags = data.word(2)?;
        let marks = if flags & 16 != 0 {
            let definition = definition.ok_or(Problem::Malformed)?;
            if definition.word(0)? != 1 || definition.word(2)? < 2 {
                return Err(Problem::Malformed);
            }
            let sets = definition.offset(12)?;
            let index = usize::from(data.word(6 + usize::from(data.word(4)?) * 2)?);
            if sets.word(0)? != 1 || index >= usize::from(sets.word(2)?) {
                return Err(Problem::Malformed);
            }
            Some(Coverage::new(
                sets.tail(sets.long(4 + index * 4)? as usize)?,
            )?)
        } else {
            None
        };
        Ok(Self {
            flags,
            definition,
            marks,
        })
    }
    pub fn skip(&self, glyph: u16) -> Result<bool, Problem> {
        let class = match self.definition {
            Some(data) if data.word(4)? != 0 => classify(data.offset(4)?, glyph)?,
            _ => 0,
        };
        Ok(match class {
            1 => self.flags & 2 != 0,
            2 => self.flags & 4 != 0,
            3 => {
                self.flags & 8 != 0
                    || self
                        .marks
                        .as_ref()
                        .is_some_and(|marks| marks.index(glyph).is_none())
            }
            _ => false,
        })
    }
}
pub(super) fn classify(data: Data<'_>, glyph: u16) -> Result<u16, Problem> {
    match data.word(0)? {
        1 => {
            let start = data.word(2)?;
            let count = data.word(4)?;
            if glyph < start || u32::from(glyph) >= u32::from(start) + u32::from(count) {
                return Ok(0);
            }
            data.word(6 + usize::from(glyph - start) * 2)
        }
        2 => ranges(data, glyph),
        _ => Err(Problem::Unsupported),
    }
}

fn ranges(data: Data<'_>, glyph: u16) -> Result<u16, Problem> {
    let count = usize::from(data.word(2)?);
    data.slice(4, count * 6)?;
    let mut low = 0;
    let mut high = count;
    while low < high {
        let mid = (low + high) / 2;
        if data.word(6 + mid * 6)? < glyph {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low < count && data.word(4 + low * 6)? <= glyph {
        data.word(8 + low * 6)
    } else {
        Ok(0)
    }
}
