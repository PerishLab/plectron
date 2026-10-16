use super::{Arguments, digest, write};
use crate::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

mod binary;

use binary::rename;

struct Instance<'a> {
    source: &'a Path,
    destination: PathBuf,
    axes: &'a [&'a str],
    family: String,
    postscript: String,
}

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
    let cjk = required(&args, "--cjk")?;
    let sources = json!({
        "arabic": origin(&args, "--arabic")?,
        "cjk": origin(&args, "--cjk")?,
    });
    let destination = PathBuf::from(args.one("--output").unwrap_or("apps/fonts"));
    let version = version(tool)?;
    std::fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    let mut assets = Vec::new();
    assets.push(build(
        tool,
        &Instance {
            source: Path::new(arabic),
            destination: destination.join("Arabic.ttf"),
            axes: &["wght=400", "wdth=100"],
            family: "Plectron Arabic".into(),
            postscript: "PlectronArabic-Regular".into(),
        },
    )?);
    for region in ["SC", "JP", "KR"] {
        let source = Path::new(cjk).join(format!("{region}.ttf"));
        assets.push(build(
            tool,
            &Instance {
                source: &source,
                destination: destination.join(format!("{region}.ttf")),
                axes: &["wght=400"],
                family: format!("Plectron CJK {region}"),
                postscript: format!("PlectronCJK{region}-Regular"),
            },
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
        "sources": sources,
        "instances": assets,
    });
    write(&destination.join("provenance.json"), &report)?;
    Ok(report)
}

fn build(tool: &str, instance: &Instance) -> Result<Value> {
    let Instance {
        source,
        destination,
        axes,
        ..
    } = instance;
    let temporary = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    let output = Command::new(tool)
        .arg("varLib.instancer")
        .arg(source)
        .args(*axes)
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
    rename(
        temporary.path(),
        destination,
        &instance.family,
        &instance.postscript,
    )?;
    Ok(json!({
        "input": source, "input_sha256": digest(source)?, "axes": axes,
        "output": destination, "output_sha256": digest(destination)?,
    }))
}

fn origin(args: &Arguments, flag: &str) -> Result<Value> {
    Ok(json!({
        "identity": required(args, &format!("{flag}-source"))?,
        "revision": required(args, &format!("{flag}-revision"))?,
        "input": required(args, flag)?,
    }))
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
    use binary::Bytes;

    #[test]
    fn generator() {
        let error = version("plectron-missing-fonttools").unwrap_err();
        assert!(error.contains("required upstream font generator"));
    }

    #[test]
    fn checksum() {
        let source = crate::root().join("apps/fonts/Lato-Regular.ttf");
        let destination = tempfile::NamedTempFile::new().unwrap();
        rename(
            &source,
            destination.path(),
            "Fixture Family",
            "Fixture-Regular",
        )
        .unwrap();
        let data = std::fs::read(destination.path()).unwrap();
        assert_eq!(Bytes(&data).checksum(), 0xB1B0_AFBA);
        let records = Bytes(&data).tables().unwrap();
        let name = records.iter().find(|table| table.tag == *b"name").unwrap();
        assert!(
            !Bytes(&data)
                .slice(name.offset, name.length)
                .unwrap()
                .is_empty()
        );
    }
}
