use super::{Problem, data::Data};
use crate::Path;
use std::{collections::BTreeMap, ops::Range, sync::Arc};

const LIMIT: usize = 16_777_216;
const COLLECTION: usize = 67_108_864;
const FACES: u32 = 64;
const SFNT: u32 = 0x0001_0000;
const TTCF: u32 = 0x7474_6366;

#[derive(Debug)]
pub struct Source {
    bytes: Arc<[u8]>,
    index: u32,
    tables: BTreeMap<[u8; 4], Range<usize>>,
    units: u16,
    ascent: i16,
    descent: i16,
    count: u16,
    metrics: u16,
    wide: bool,
}
impl Source {
    pub fn faces(bytes: &[u8]) -> Result<u32, Problem> {
        let data = Data(bytes);
        match data.long(0)? {
            SFNT => {
                if bytes.len() > LIMIT {
                    return Err(Problem::Budget);
                }
                Ok(1)
            }
            TTCF => {
                if bytes.len() > COLLECTION {
                    return Err(Problem::Budget);
                }
                if !matches!(data.long(4)?, 0x0001_0000 | 0x0002_0000) {
                    return Err(Problem::Unsupported);
                }
                let count = data.long(8)?;
                if count == 0 {
                    return Err(Problem::Malformed);
                }
                if count > FACES {
                    return Err(Problem::Budget);
                }
                let total = usize::try_from(count)
                    .ok()
                    .and_then(|value| value.checked_mul(4))
                    .ok_or(Problem::Malformed)?;
                data.slice(12, total)?;
                Ok(count)
            }
            _ => Err(Problem::Unsupported),
        }
    }
    pub fn parse(bytes: Arc<[u8]>) -> Result<Self, Problem> {
        Self::face(bytes, 0)
    }
    pub fn face(bytes: Arc<[u8]>, index: u32) -> Result<Self, Problem> {
        let data = Data(&bytes);
        let origin = locate(data, index)?;
        let mut tables = BTreeMap::new();
        for item in 0..usize::from(data.word(origin + 4)?) {
            let offset = origin
                .checked_add(12)
                .and_then(|value| value.checked_add(item * 16))
                .ok_or(Problem::Malformed)?;
            let tag = data
                .slice(offset, 4)?
                .0
                .try_into()
                .map_err(|_| Problem::Malformed)?;
            let start = data.long(offset + 8)? as usize;
            let size = data.long(offset + 12)? as usize;
            data.slice(start, size)?;
            if tables.insert(tag, start..start + size).is_some() {
                return Err(Problem::Malformed);
            }
        }
        if [b"fvar", b"CFF ", b"CFF2", b"COLR", b"CBDT", b"sbix"]
            .iter()
            .any(|tag| tables.contains_key(*tag))
        {
            return Err(Problem::Unsupported);
        }
        let mut source = Self {
            bytes,
            index,
            tables,
            units: 0,
            ascent: 0,
            descent: 0,
            count: 0,
            metrics: 0,
            wide: false,
        };
        let head = source.table(b"head")?;
        if head.long(12)? != 0x5f0f3cf5 || head.signed(52)? != 0 {
            return Err(Problem::Malformed);
        }
        let units = head.word(18)?;
        let wide = match head.signed(50)? {
            0 => false,
            1 => true,
            _ => return Err(Problem::Malformed),
        };
        if !(16..=16384).contains(&units) {
            return Err(Problem::Malformed);
        }
        source.units = units;
        source.wide = wide;
        source.count = source.table(b"maxp")?.word(4)?;
        source.ascent = source.table(b"hhea")?.signed(4)?;
        source.descent = source.table(b"hhea")?.signed(6)?;
        source.metrics = source.table(b"hhea")?.word(34)?;
        if source.metrics == 0 || source.metrics > source.count || source.ascent <= source.descent {
            return Err(Problem::Malformed);
        }
        source.table(b"hmtx")?.slice(
            0,
            usize::from(source.metrics) * 4 + usize::from(source.count - source.metrics) * 2,
        )?;
        let mut previous = 0;
        for glyph in 0..=source.count {
            let offset = source.location(glyph)?;
            if offset < previous || offset > source.table(b"glyf")?.0.len() {
                return Err(Problem::Malformed);
            }
            previous = offset;
        }
        source.mapping()?;
        Ok(source)
    }
    pub(crate) fn optional(&self, tag: &[u8; 4]) -> Option<Data<'_>> {
        self.tables
            .get(tag)
            .map(|range| Data(&self.bytes[range.clone()]))
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn slot(&self) -> u32 {
        self.index
    }
    pub fn units(&self) -> u16 {
        self.units
    }
    pub fn ascent(&self) -> i16 {
        self.ascent
    }
    pub fn descent(&self) -> i16 {
        self.descent
    }
    pub fn count(&self) -> u16 {
        self.count
    }
    pub fn advance(&self, glyph: u16) -> Result<u16, Problem> {
        self.check(glyph)?;
        self.table(b"hmtx")?
            .word(usize::from(glyph.min(self.metrics - 1)) * 4)
    }
    pub fn outline(&self, glyph: u16) -> Result<Path, Problem> {
        let mut budget = 65_536;
        self.contours(glyph, &mut Vec::new(), &mut budget)?.path()
    }
    pub(super) fn check(&self, glyph: u16) -> Result<(), Problem> {
        if glyph >= self.count {
            Err(Problem::Missing)
        } else {
            Ok(())
        }
    }
    pub(crate) fn table(&self, tag: &[u8; 4]) -> Result<Data<'_>, Problem> {
        let range = self.tables.get(tag).ok_or(Problem::Malformed)?;
        Ok(Data(&self.bytes[range.clone()]))
    }
    fn location(&self, glyph: u16) -> Result<usize, Problem> {
        let data = self.table(b"loca")?;
        if self.wide {
            Ok(data.long(usize::from(glyph) * 4)? as usize)
        } else {
            Ok(usize::from(data.word(usize::from(glyph) * 2)?) * 2)
        }
    }
    pub(super) fn glyph(&self, glyph: u16) -> Result<Data<'_>, Problem> {
        self.check(glyph)?;
        let start = self.location(glyph)?;
        let end = self.location(glyph + 1)?;
        self.table(b"glyf")?.slice(start, end - start)
    }
}

fn locate(data: Data<'_>, index: u32) -> Result<usize, Problem> {
    let count = Source::faces(data.0)?;
    if index >= count {
        return Err(Problem::Missing);
    }
    if data.long(0)? == SFNT {
        return Ok(0);
    }
    let slot = usize::try_from(index)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .and_then(|value| value.checked_add(12))
        .ok_or(Problem::Malformed)?;
    let offset = data.long(slot)? as usize;
    if data.long(offset)? != SFNT {
        return Err(Problem::Unsupported);
    }
    Ok(offset)
}
