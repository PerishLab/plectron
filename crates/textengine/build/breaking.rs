use std::{collections::HashMap, fmt::Write, path::Path};

const LIMIT: usize = 0x110000;
const WIDTH: usize = 256;
type Rows = Vec<(usize, usize, String)>;
type Failure = Box<dyn std::error::Error>;
const CLASSES: &[(&str, &str)] = &[
    ("AL", "Al"),
    ("AI", "Ai"),
    ("AK", "Ak"),
    ("AP", "Ap"),
    ("AS", "As"),
    ("B2", "B2"),
    ("BA", "Ba"),
    ("BB", "Bb"),
    ("BK", "Bk"),
    ("CB", "Cb"),
    ("CJ", "Cj"),
    ("CL", "Cl"),
    ("CM", "Cm"),
    ("CP", "Cp"),
    ("CR", "Cr"),
    ("EB", "Eb"),
    ("EM", "Em"),
    ("EX", "Ex"),
    ("GL", "Gl"),
    ("H2", "H2"),
    ("H3", "H3"),
    ("HH", "Hh"),
    ("HL", "Hl"),
    ("HY", "Hy"),
    ("ID", "Id"),
    ("IN", "In"),
    ("IS", "Is"),
    ("JL", "Jl"),
    ("JT", "Jt"),
    ("JV", "Jv"),
    ("LF", "Lf"),
    ("NL", "Nl"),
    ("NS", "Ns"),
    ("NU", "Nu"),
    ("OP", "Op"),
    ("PO", "Po"),
    ("PR", "Pr"),
    ("QU", "Qu"),
    ("RI", "Ri"),
    ("SA", "Sa"),
    ("SG", "Sg"),
    ("SP", "Sp"),
    ("SY", "Sy"),
    ("VF", "Vf"),
    ("VI", "Vi"),
    ("WJ", "Wj"),
    ("XX", "Xx"),
    ("ZW", "Zw"),
    ("ZWJ", "Zwj"),
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Cell {
    class: u8,
    flags: u8,
}

pub fn generate(root: &Path, output: &Path) -> Result<(), Failure> {
    let mut table = vec![Cell { class: 0, flags: 0 }; LIMIT];
    for (start, end, name) in ranges(&root.join("LineBreak.txt"))? {
        let class = CLASSES
            .iter()
            .position(|entry| entry.0 == name)
            .ok_or_else(|| format!("unknown line class {name}"))? as u8;
        for cell in &mut table[start..=end] {
            cell.class = class;
        }
    }
    for (start, end, name) in ranges(&root.join("EastAsianWidth.txt"))? {
        if matches!(name.as_str(), "F" | "W" | "H") {
            for cell in &mut table[start..=end] {
                cell.flags |= 1;
            }
        }
    }
    for (start, end, name) in ranges(&root.join("DerivedGeneralCategory.txt"))? {
        let flags = u8::from(name == "Pi") << 1
            | u8::from(name == "Pf") << 2
            | u8::from(matches!(name.as_str(), "Mn" | "Mc")) << 3
            | u8::from(name.starts_with(['L', 'N'])) << 4
            | u8::from(name == "Zs") << 5
            | u8::from(name == "Cn") << 6;
        for cell in &mut table[start..=end] {
            cell.flags |= flags;
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
            bytes.extend([cell.class, cell.flags]);
        }
    }
    std::fs::write(output.join("breaking.bin"), bytes)?;
    let mut source = String::new();
    source.push_str(
        "const BREAKING: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/breaking.bin\"));\n",
    );
    source.push_str("fn generated(character: char) -> Property {\n");
    source.push_str("let code = character as usize;\n");
    source.push_str("let slot = code / 256 * 2;\n");
    source.push_str(
        "let page = u16::from_le_bytes([BREAKING[slot], BREAKING[slot + 1]]) as usize;\n",
    );
    let _ = writeln!(
        source,
        "let offset = {base} + (page * 256 + code % 256) * 2;"
    );
    source.push_str("let class = match BREAKING[offset] {\n");
    for (code, (_, variant)) in CLASSES.iter().enumerate() {
        let _ = writeln!(source, "{code} => Class::{variant},");
    }
    source.push_str("_ => unreachable!(),\n};\n");
    source.push_str("let flags = BREAKING[offset + 1];\n");
    source.push_str("Property { class, east: flags & 1 != 0, initial: flags & 2 != 0, finality: flags & 4 != 0, mark: flags & 8 != 0, letter: flags & 16 != 0, separator: flags & 32 != 0, unassigned: flags & 64 != 0 }\n}\n");
    std::fs::write(output.join("breaking.rs"), source)?;
    Ok(())
}

fn ranges(path: &Path) -> Result<Rows, Failure> {
    let source = std::fs::read_to_string(path)?;
    let mut result = Vec::new();
    for line in source.lines() {
        let Some((bounds, value)) = line.split('#').next().unwrap().split_once(';') else {
            continue;
        };
        let mut bounds = bounds.trim().split("..");
        let start = usize::from_str_radix(bounds.next().unwrap(), 16)?;
        let end = bounds
            .next()
            .map(|value| usize::from_str_radix(value, 16))
            .transpose()?
            .unwrap_or(start);
        result.push((start, end, value.trim().into()));
    }
    Ok(result)
}
