use super::{Problem, data::Data};
use crate::Path;
use std::{collections::BTreeMap, ops::Range, sync::Arc};

#[derive(Debug)]
pub struct Source {
    bytes: Arc<[u8]>,
    tables: BTreeMap<[u8; 4], Range<usize>>,
    units: u16,
    ascent: i16,
    descent: i16,
    count: u16,
    metrics: u16,
    wide: bool,
}
impl Source {
    pub fn parse(bytes: Arc<[u8]>) -> Result<Self, Problem> {
        let data = Data(&bytes);
        if data.long(0)? != 0x00010000 {
            return Err(Problem::Unsupported);
        }
        if bytes.len() > 16_777_216 {
            return Err(Problem::Budget);
        }
        let mut tables = BTreeMap::new();
        for index in 0..usize::from(data.word(4)?) {
            let offset = 12 + index * 16;
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
