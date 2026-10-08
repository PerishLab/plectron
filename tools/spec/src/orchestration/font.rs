use super::{Arguments, digest, write};
use crate::Result;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const IDS: [u16; 7] = [1, 2, 3, 4, 6, 16, 17];

pub fn run(values: &[String]) -> Result<Value> {
    let args = Arguments::new(values);
    args.reject(&[
        "--fonttools",
        "--arabic",
        "--arabic-source",
        "--arabic-revision",
        "--cjk",
        "--cjk-source",
        "--cjk-revision",
        "--output",
    ])?;
    let tool = args.one("--fonttools").unwrap_or("fonttools");
    let arabic = required(&args, "--arabic")?;
    let arabic_source = required(&args, "--arabic-source")?;
    let arabic_revision = required(&args, "--arabic-revision")?;
    let cjk = required(&args, "--cjk")?;
    let cjk_source = required(&args, "--cjk-source")?;
    let cjk_revision = required(&args, "--cjk-revision")?;
    let destination = PathBuf::from(args.one("--output").unwrap_or("apps/fonts"));
    let version = version(tool)?;
    std::fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    let mut assets = Vec::new();
    assets.push(build(
        tool,
        Path::new(arabic),
        &destination.join("Arabic.ttf"),
        &["wght=400", "wdth=100"],
        "Plectron Arabic",
        "PlectronArabic-Regular",
    )?);
    for region in ["SC", "JP", "KR"] {
        assets.push(build(
            tool,
            &Path::new(cjk).join(format!("{region}.ttf")),
            &destination.join(format!("{region}.ttf")),
            &["wght=400"],
            &format!("Plectron CJK {region}"),
            &format!("PlectronCJK{region}-Regular"),
        )?);
    }
    copy(
        &Path::new(arabic).with_file_name("OFL.txt"),
        &destination.join("Arabic-OFL.txt"),
    )?;
    copy(
        &Path::new(cjk).join("OFL.txt"),
        &destination.join("CJK-OFL.txt"),
    )?;
    let report = json!({
        "generator": {"command": tool, "version": version},
        "sources": {
            "arabic": {"identity": arabic_source,
                "revision": arabic_revision, "input": arabic},
            "cjk": {"identity": cjk_source,
                "revision": cjk_revision, "input": cjk},
        },
        "instances": assets,
    });
    write(&destination.join("provenance.json"), &report)?;
    Ok(report)
}

fn build(
    tool: &str,
    source: &Path,
    destination: &Path,
    axes: &[&str],
    family: &str,
    postscript: &str,
) -> Result<Value> {
    let temporary = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    let output = Command::new(tool)
        .arg("varLib.instancer")
        .arg(source)
        .args(axes)
        .arg("--no-recalc-timestamp")
        .arg("--output")
        .arg(temporary.path())
        .output()
        .map_err(|error| prerequisite(tool, error))?;
    if !output.status.success() {
        return Err(format!(
            "fontTools instancing failed for {}: {}",
            source.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    rename(temporary.path(), destination, family, postscript)?;
    Ok(json!({
        "input": source, "input_sha256": digest(source)?, "axes": axes,
        "output": destination, "output_sha256": digest(destination)?,
    }))
}

fn rename(source: &Path, destination: &Path, family: &str, postscript: &str) -> Result<()> {
    let data = std::fs::read(source).map_err(|error| error.to_string())?;
    let tables = tables(&data)?;
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
    let replacement = name_table(slice(&data, name.offset, name.length)?, &names)?;
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
            slice(&data, table.offset, table.length)?.to_vec()
        };
        if table.tag == *b"head" {
            bytes.get_mut(8..12).ok_or("invalid head table")?.fill(0);
        }
        output.extend_from_slice(&bytes);
        rewritten.push((
            table.tag,
            offset,
            u32::try_from(bytes.len()).map_err(|_| "table is too large")?,
        ));
    }
    for (index, (tag, offset, length)) in rewritten.iter().enumerate() {
        let record = 12 + index * 16;
        let table_checksum = checksum(slice(&output, *offset, *length)?);
        output[record..record + 4].copy_from_slice(tag);
        put(&mut output, record + 4, table_checksum);
        put(&mut output, record + 8, *offset);
        put(&mut output, record + 12, *length);
    }
    if let Some((_, offset, _)) = rewritten.iter().find(|(tag, _, _)| tag == b"head") {
        let adjustment = usize::try_from(*offset).map_err(|_| "head offset is too large")? + 8;
        put(&mut output, adjustment, 0);
        let whole = checksum(&output);
        put(&mut output, adjustment, 0xB1B0_AFBAu32.wrapping_sub(whole));
    }
    std::fs::write(destination, output).map_err(|error| error.to_string())
}

fn name_table(data: &[u8], names: &BTreeMap<u16, String>) -> Result<Vec<u8>> {
    if word(data, 0)? != 0 {
        return Err("font uses an unsupported versioned name table".into());
    }
    let count = usize::from(word(data, 2)?);
    let strings = usize::from(word(data, 4)?);
    if data.len() < 6 + count * 12 || strings > data.len() {
        return Err("invalid name table".into());
    }
    let mut output = data[..6 + count * 12].to_vec();
    output.truncate(6 + count * 12);
    let mut storage = Vec::new();
    for index in 0..count {
        let record = 6 + index * 12;
        let platform = word(data, record)?;
        let id = word(data, record + 6)?;
        let length = usize::from(word(data, record + 8)?);
        let offset = usize::from(word(data, record + 10)?);
        let bytes = if IDS.contains(&id) {
            let value = names.get(&id).ok_or("missing replacement name")?;
            if matches!(platform, 0 | 3) {
                value.encode_utf16().flat_map(u16::to_be_bytes).collect()
            } else {
                value.as_bytes().to_vec()
            }
        } else {
            data.get(strings + offset..strings + offset + length)
                .ok_or("invalid name string")?
                .to_vec()
        };
        let new_offset = u16::try_from(storage.len()).map_err(|_| "name storage is too large")?;
        let new_length = u16::try_from(bytes.len()).map_err(|_| "name is too large")?;
        output[record + 8..record + 10].copy_from_slice(&new_length.to_be_bytes());
        output[record + 10..record + 12].copy_from_slice(&new_offset.to_be_bytes());
        storage.extend(bytes);
    }
    let string_offset = u16::try_from(output.len()).map_err(|_| "name table is too large")?;
    output[4..6].copy_from_slice(&string_offset.to_be_bytes());
    output.extend(storage);
    Ok(output)
}

#[derive(Clone)]
struct Table {
    tag: [u8; 4],
    offset: u32,
    length: u32,
}

fn tables(data: &[u8]) -> Result<Vec<Table>> {
    let count = usize::from(word(data, 4)?);
    (0..count)
        .map(|index| {
            let record = 12 + index * 16;
            Ok(Table {
                tag: data
                    .get(record..record + 4)
                    .ok_or("invalid table record")?
                    .try_into()
                    .unwrap(),
                offset: long(data, record + 8)?,
                length: long(data, record + 12)?,
            })
        })
        .collect()
}

fn slice(data: &[u8], offset: u32, length: u32) -> Result<&[u8]> {
    let start = usize::try_from(offset).map_err(|_| "table offset is too large")?;
    let length = usize::try_from(length).map_err(|_| "table length is too large")?;
    data.get(start..start + length)
        .ok_or_else(|| "invalid table bounds".into())
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn word(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(
        data.get(offset..offset + 2)
            .ok_or("invalid font offset")?
            .try_into()
            .unwrap(),
    ))
}

fn long(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        data.get(offset..offset + 4)
            .ok_or("invalid font offset")?
            .try_into()
            .unwrap(),
    ))
}

fn put(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn copy(source: &Path, destination: &Path) -> Result<()> {
    std::fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", source.display()))
}

fn version(tool: &str) -> Result<String> {
    let output = Command::new(tool)
        .arg("--version")
        .output()
        .map_err(|error| prerequisite(tool, error))?;
    super::version(tool, output)
}

fn required<'a>(args: &Arguments<'a>, name: &str) -> Result<&'a str> {
    args.one(name).ok_or_else(|| format!("{name} is required"))
}

fn prerequisite(tool: &str, error: std::io::Error) -> String {
    format!("required upstream font generator {tool} is unavailable: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_generator_is_explicit() {
        let error = version("plectron-missing-fonttools").unwrap_err();
        assert!(error.contains("required upstream font generator"));
    }

    #[test]
    fn rebuilt_font_has_a_valid_checksum() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/fonts/Lato-Regular.ttf");
        let destination = tempfile::NamedTempFile::new().unwrap();
        rename(
            &source,
            destination.path(),
            "Fixture Family",
            "Fixture-Regular",
        )
        .unwrap();
        let data = std::fs::read(destination.path()).unwrap();
        assert_eq!(checksum(&data), 0xB1B0_AFBA);
        let records = tables(&data).unwrap();
        let name = records.iter().find(|table| table.tag == *b"name").unwrap();
        assert!(!slice(&data, name.offset, name.length).unwrap().is_empty());
    }
}
