use std::{collections::HashMap, fmt::Write, path::Path};

const LIMIT: usize = 0x110000;
const WIDTH: usize = 256;
const PROPERTIES: &[(&str, &str)] = &[
    ("Other", "Other"),
    ("CR", "Cr"),
    ("LF", "Lf"),
    ("Control", "Control"),
    ("Extend", "Extend"),
    ("ZWJ", "Zwj"),
    ("SpacingMark", "Spacing"),
    ("Prepend", "Prepend"),
    ("Regional_Indicator", "Regional"),
    ("L", "L"),
    ("V", "V"),
    ("T", "T"),
    ("LV", "Lv"),
    ("LVT", "Lvt"),
    ("Consonant", "Consonant"),
    ("Linker", "Linker"),
    ("Extended_Pictographic", "Picture"),
];
type Failure = Box<dyn std::error::Error>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Cell([u8; 2]);

struct Row {
    start: usize,
    end: usize,
    values: Vec<String>,
}

pub fn generate(root: &Path, output: &Path) -> Result<(), Failure> {
    let mut table = vec![Cell([0, 0]); LIMIT];
    for row in rows(&root.join("GraphemeBreakProperty.txt"))? {
        let property = code(&row.values[0])?;
        for cell in &mut table[row.start..=row.end] {
            cell.0[0] = property;
        }
    }
    for row in rows(&root.join("DerivedCoreProperties.txt"))? {
        if row.values[0] == "InCB" {
            let property = code(&row.values[1])?;
            for cell in &mut table[row.start..=row.end] {
                cell.0[1] = property;
            }
        }
    }
    for row in rows(&root.join("emoji-data.txt"))? {
        if row.values[0] == "Extended_Pictographic" {
            for cell in &mut table[row.start..=row.end] {
                cell.0[1] |= 0x80;
            }
        }
    }
    let mut pages = Vec::<Vec<Cell>>::new();
    let mut known = HashMap::<Vec<Cell>, u16>::new();
    let mut index = Vec::new();
    for page in table.chunks_exact(WIDTH) {
        let page = page.to_vec();
        let next = u16::try_from(pages.len())?;
        let id = *known.entry(page.clone()).or_insert_with(|| {
            pages.push(page);
            next
        });
        index.push(id);
    }
    let base = index.len() * 2;
    let mut bytes = Vec::with_capacity(base + pages.len() * WIDTH * 2);
    for id in index {
        bytes.extend(id.to_le_bytes());
    }
    for page in pages {
        for cell in page {
            bytes.extend(cell.0);
        }
    }
    std::fs::write(output.join("grapheme.bin"), bytes)?;
    let mut source = String::new();
    source.push_str(
        "const GRAPHEME: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/grapheme.bin\"));\n",
    );
    source.push_str("impl Tables {\npub fn get() -> &'static Self { static TABLES: Tables = Tables; &TABLES }\n");
    source.push_str("pub fn properties(&self, character: char) -> Attributes {\n");
    source.push_str("let code = character as usize; let slot = code / 256 * 2;\n");
    source.push_str(
        "let page = u16::from_le_bytes([GRAPHEME[slot], GRAPHEME[slot + 1]]) as usize;\n",
    );
    let _ = writeln!(
        source,
        "let offset = {base} + (page * 256 + code % 256) * 2;"
    );
    source.push_str("(decode(GRAPHEME[offset]), decode(GRAPHEME[offset + 1] & 0x7f), GRAPHEME[offset + 1] & 0x80 != 0)\n}\n");
    source.push_str(
        "pub fn picture(&self, character: char) -> bool { self.properties(character).2 }\n}\n",
    );
    source.push_str("fn decode(value: u8) -> Property { match value {\n");
    for (value, (_, variant)) in PROPERTIES.iter().enumerate() {
        let _ = writeln!(source, "{value} => Property::{variant},");
    }
    source.push_str("_ => unreachable!(),\n} }\n");
    std::fs::write(output.join("grapheme.rs"), source)?;
    Ok(())
}

fn code(name: &str) -> Result<u8, Failure> {
    PROPERTIES
        .iter()
        .position(|entry| entry.0 == name)
        .map(|value| value as u8)
        .ok_or_else(|| format!("unknown grapheme property {name}").into())
}

fn rows(path: &Path) -> Result<Vec<Row>, Failure> {
    let source = std::fs::read_to_string(path)?;
    let mut result = Vec::new();
    for line in source.lines() {
        let mut parts = line.split('#').next().unwrap().split(';').map(str::trim);
        let Some(bounds) = parts.next().filter(|value| !value.is_empty()) else {
            continue;
        };
        let values: Vec<_> = parts.map(Into::into).collect();
        if values.is_empty() {
            continue;
        }
        let mut bounds = bounds.split("..");
        let start = usize::from_str_radix(bounds.next().unwrap(), 16)?;
        let end = bounds
            .next()
            .map(|value| usize::from_str_radix(value, 16))
            .transpose()?
            .unwrap_or(start);
        result.push(Row { start, end, values });
    }
    Ok(result)
}
