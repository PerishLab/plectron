use crate::font::{Problem, data::Data};

#[derive(Clone, Default)]
pub(super) struct Coverage<'a> {
    records: &'a [u8],
    stride: usize,
    length: usize,
}
pub(super) struct Record {
    range: std::ops::Range<usize>,
    stride: usize,
    length: usize,
}
impl Record {
    pub fn view<'a>(&self, source: &'a [u8]) -> Coverage<'a> {
        Coverage {
            records: &source[self.range.clone()],
            stride: self.stride,
            length: self.length,
        }
    }
}
impl<'a> Coverage<'a> {
    pub fn record(&self, source: &[u8]) -> Record {
        Record {
            range: Data(self.records).range(source),
            stride: self.stride,
            length: self.length,
        }
    }
    pub fn new(data: Data<'a>) -> Result<Self, Problem> {
        let stride = match data.word(0)? {
            1 => 2,
            2 => 6,
            _ => return Err(Problem::Unsupported),
        };
        let count = usize::from(data.word(2)?);
        let records = data.slice(4, count * stride)?.0;
        let mut length = 0;
        let mut previous = None;
        for record in records.chunks_exact(stride) {
            let start = word(record, 0);
            let end = word(record, if stride == 6 { 2 } else { 0 });
            if start > end || previous.is_some_and(|previous| previous >= start) {
                return Err(Problem::Malformed);
            }
            if stride == 6 && usize::from(word(record, 4)) != length {
                return Err(Problem::Malformed);
            }
            length += usize::from(end - start) + 1;
            if length > 65_536 {
                return Err(Problem::Budget);
            }
            previous = Some(end);
        }
        Ok(Self {
            records,
            stride,
            length,
        })
    }
    pub fn len(&self) -> usize {
        self.length
    }
    pub fn index(&self, glyph: u16) -> Option<usize> {
        if self.records.is_empty() {
            return None;
        }
        let mut low = 0;
        let mut high = self.records.len() / self.stride;
        while low < high {
            let middle = (low + high) / 2;
            let offset = middle * self.stride;
            let end = word(self.records, offset + if self.stride == 6 { 2 } else { 0 });
            if end < glyph {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let record = self.records.get(low * self.stride..)?.get(..self.stride)?;
        let start = word(record, 0);
        if start > glyph {
            return None;
        }
        let index = if self.stride == 6 {
            usize::from(word(record, 4))
        } else {
            low
        };
        Some(index + usize::from(glyph - start))
    }
}
fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}
