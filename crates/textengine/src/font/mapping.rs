use super::{Problem, Source, data::Data};

impl Source {
    pub(super) fn mapping(&self) -> Result<Data<'_>, Problem> {
        let table = self.table(b"cmap")?;
        let mut selected = None;
        for index in 0..usize::from(table.word(2)?) {
            let offset = 4 + index * 8;
            let platform = table.word(offset)?;
            let encoding = table.word(offset + 2)?;
            if platform != 0 && !(platform == 3 && matches!(encoding, 1 | 10)) {
                continue;
            }
            let start = table.long(offset + 4)? as usize;
            let format = table.word(start)?;
            let size = match format {
                4 => usize::from(table.word(start + 2)?),
                12 => table.long(start + 4)? as usize,
                _ => continue,
            };
            let subtable = table.slice(start, size)?;
            if selected.is_none() || format == 12 {
                selected = Some(subtable);
            }
        }
        selected.ok_or(Problem::Unsupported)
    }
    pub fn index(&self, character: char) -> Result<Option<u16>, Problem> {
        let data = self.mapping()?;
        let glyph = match data.word(0)? {
            4 => compact(data, character as u32)?,
            12 => extended(data, character as u32)?,
            _ => return Err(Problem::Unsupported),
        };
        if glyph == 0 {
            return Ok(None);
        }
        let glyph = u16::try_from(glyph).map_err(|_| Problem::Malformed)?;
        if glyph >= self.count() {
            return Err(Problem::Malformed);
        }
        Ok(Some(glyph))
    }
}
fn compact(data: Data<'_>, character: u32) -> Result<u32, Problem> {
    if character > 65535 {
        return Ok(0);
    }
    let count = usize::from(data.word(6)?) / 2;
    if count == 0 || data.word(6)? % 2 != 0 {
        return Err(Problem::Malformed);
    }
    data.slice(14, count * 8 + 2)?;
    for index in 0..count {
        let end = u32::from(data.word(14 + index * 2)?);
        if character > end {
            continue;
        }
        let start = u32::from(data.word(16 + count * 2 + index * 2)?);
        if character < start {
            return Ok(0);
        }
        let delta = data.word(16 + count * 4 + index * 2)?;
        let position = 16 + count * 6 + index * 2;
        let offset = usize::from(data.word(position)?);
        if offset == 0 {
            return Ok(u32::from((character as u16).wrapping_add(delta)));
        }
        let glyph = data.word(position + offset + (character - start) as usize * 2)?;
        return Ok(if glyph == 0 {
            0
        } else {
            u32::from(glyph.wrapping_add(delta))
        });
    }
    Ok(0)
}
fn extended(data: Data<'_>, character: u32) -> Result<u32, Problem> {
    let count = data.long(12)? as usize;
    data.slice(16, count.checked_mul(12).ok_or(Problem::Malformed)?)?;
    for index in 0..count {
        let offset = 16 + index * 12;
        let start = data.long(offset)?;
        let end = data.long(offset + 4)?;
        if start > end {
            return Err(Problem::Malformed);
        }
        if (start..=end).contains(&character) {
            return data
                .long(offset + 8)?
                .checked_add(character - start)
                .ok_or(Problem::Malformed);
        }
    }
    Ok(0)
}
