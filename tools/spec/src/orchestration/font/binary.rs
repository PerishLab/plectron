use crate::Result;
use std::collections::BTreeMap;
use std::path::Path;

const IDS: [u16; 7] = [1, 2, 3, 4, 6, 16, 17];

#[derive(Clone)]
pub(super) struct Table {
    pub(super) tag: [u8; 4],
    pub(super) offset: u32,
    pub(super) length: u32,
}

#[derive(Clone, Copy)]
pub(super) struct Bytes<'a>(pub(super) &'a [u8]);

impl<'a> Bytes<'a> {
    pub(super) fn tables(self) -> Result<Vec<Table>> {
        let count = usize::from(self.word(4)?);
        (0..count)
            .map(|index| {
                let record = 12 + index * 16;
                Ok(Table {
                    tag: self
                        .0
                        .get(record..record + 4)
                        .ok_or("invalid table record")?
                        .try_into()
                        .unwrap(),
                    offset: self.long(record + 8)?,
                    length: self.long(record + 12)?,
                })
            })
            .collect()
    }

    pub(super) fn slice(self, offset: u32, length: u32) -> Result<&'a [u8]> {
        let start = usize::try_from(offset).map_err(|_| "table offset is too large")?;
        let length = usize::try_from(length).map_err(|_| "table length is too large")?;
        self.0
            .get(start..start + length)
            .ok_or_else(|| "invalid table bounds".into())
    }

    pub(super) fn checksum(self) -> u32 {
        self.0.chunks(4).fold(0u32, |sum, chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            sum.wrapping_add(u32::from_be_bytes(word))
        })
    }

    fn word(self, offset: usize) -> Result<u16> {
        Ok(u16::from_be_bytes(
            self.0
                .get(offset..offset + 2)
                .ok_or("invalid font offset")?
                .try_into()
                .unwrap(),
        ))
    }

    fn long(self, offset: usize) -> Result<u32> {
        Ok(u32::from_be_bytes(
            self.0
                .get(offset..offset + 4)
                .ok_or("invalid font offset")?
                .try_into()
                .unwrap(),
        ))
    }

    fn string(
        self,
        record: usize,
        strings: usize,
        names: &BTreeMap<u16, String>,
    ) -> Result<Vec<u8>> {
        let platform = self.word(record)?;
        let id = self.word(record + 6)?;
        if IDS.contains(&id) {
            let value = names.get(&id).ok_or("missing replacement name")?;
            return Ok(if matches!(platform, 0 | 3) {
                value.encode_utf16().flat_map(u16::to_be_bytes).collect()
            } else {
                value.as_bytes().to_vec()
            });
        }
        let length = usize::from(self.word(record + 8)?);
        let offset = usize::from(self.word(record + 10)?);
        Ok(self
            .0
            .get(strings + offset..strings + offset + length)
            .ok_or("invalid name string")?
            .to_vec())
    }

    fn names(self, names: &BTreeMap<u16, String>) -> Result<Vec<u8>> {
        if self.word(0)? != 0 {
            return Err("font uses an unsupported versioned name table".into());
        }
        let count = usize::from(self.word(2)?);
        let strings = usize::from(self.word(4)?);
        if self.0.len() < 6 + count * 12 || strings > self.0.len() {
            return Err("invalid name table".into());
        }
        let mut output = self.0[..6 + count * 12].to_vec();
        let mut storage = Vec::new();
        for index in 0..count {
            let record = 6 + index * 12;
            let bytes = self.string(record, strings, names)?;
            let position = u16::try_from(storage.len()).map_err(|_| "name storage is too large")?;
            let size = u16::try_from(bytes.len()).map_err(|_| "name is too large")?;
            output[record + 8..record + 10].copy_from_slice(&size.to_be_bytes());
            output[record + 10..record + 12].copy_from_slice(&position.to_be_bytes());
            storage.extend(bytes);
        }
        let start = u16::try_from(output.len()).map_err(|_| "name table is too large")?;
        output[4..6].copy_from_slice(&start.to_be_bytes());
        output.extend(storage);
        Ok(output)
    }
}

pub(super) fn rename(
    source: &Path,
    destination: &Path,
    family: &str,
    postscript: &str,
) -> Result<()> {
    let data = std::fs::read(source).map_err(|error| error.to_string())?;
    let font = Bytes(&data);
    let tables = font.tables()?;
    let name = tables
        .iter()
        .find(|table| table.tag == *b"name")
        .ok_or("font has no name table")?;
    let names = BTreeMap::from([
        (1, family.to_owned()),
        (2, "Regular".into()),
        (3, postscript.to_owned()),
        (4, format!("{family} Regular")),
        (6, postscript.to_owned()),
        (16, family.to_owned()),
        (17, "Regular".into()),
    ]);
    let replacement = Bytes(font.slice(name.offset, name.length)?).names(&names)?;
    let mut output = data[..12].to_vec();
    output.resize(12 + tables.len() * 16, 0);
    let mut rewritten = Vec::new();
    for table in &tables {
        while output.len() % 4 != 0 {
            output.push(0);
        }
        let offset = u32::try_from(output.len()).map_err(|_| "font is too large")?;
        let mut bytes = if table.tag == *b"name" {
            replacement.clone()
        } else {
            font.slice(table.offset, table.length)?.to_vec()
        };
        if table.tag == *b"head" {
            bytes.get_mut(8..12).ok_or("invalid head table")?.fill(0);
        }
        output.extend_from_slice(&bytes);
        let length = u32::try_from(bytes.len()).map_err(|_| "table is too large")?;
        rewritten.push(Table {
            tag: table.tag,
            offset,
            length,
        });
    }
    for (index, table) in rewritten.iter().enumerate() {
        let record = 12 + index * 16;
        let sum = Bytes(&output).slice(table.offset, table.length)?;
        let sum = Bytes(sum).checksum();
        output[record..record + 4].copy_from_slice(&table.tag);
        put(&mut output, record + 4, sum);
        put(&mut output, record + 8, table.offset);
        put(&mut output, record + 12, table.length);
    }
    if let Some(head) = rewritten.iter().find(|table| table.tag == *b"head") {
        let adjustment = usize::try_from(head.offset).map_err(|_| "head offset is too large")? + 8;
        put(&mut output, adjustment, 0);
        let whole = Bytes(&output).checksum();
        put(&mut output, adjustment, 0xB1B0_AFBAu32.wrapping_sub(whole));
    }
    std::fs::write(destination, output).map_err(|error| error.to_string())
}

fn put(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
