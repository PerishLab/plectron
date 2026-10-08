use super::{Arguments, digest, write};
use crate::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

const ARABIC: [&str; 8] = [
    "سلام",
    "السلام عليكم",
    "مرحبا بالعالم",
    "العَرَبِيَّة",
    "لا لأ لإ لآ",
    "ببب ب\u{200c}ب ب\u{200d}ب",
    "بِسْمِ الله",
    "فارسی اردو",
];

pub fn run(values: &[String]) -> Result<Value> {
    let args = Arguments::new(values);
    args.reject(&[
        "--hb-shape",
        "--fonts",
        "--ranges",
        "--output",
        "--provenance",
        "--source",
        "--revision",
    ])?;
    let tool = args.one("--hb-shape").unwrap_or("hb-shape");
    let fonts = PathBuf::from(args.one("--fonts").unwrap_or("apps/fonts"));
    let output = PathBuf::from(required(&args, "--output")?);
    let provenance = PathBuf::from(required(&args, "--provenance")?);
    let source = required(&args, "--source")?;
    let revision = required(&args, "--revision")?;
    let version = command_version(tool)?;
    let ranges = values.iter().any(|value| value == "--ranges");
    let samples = if ranges {
        range_samples()
    } else {
        arabic_samples()
    };
    let mut shaped = Vec::new();
    for sample in samples {
        shaped.push(shape(tool, &fonts, &sample)?);
    }
    write(&output, &json!(shaped))?;
    let faces = ["Arabic", "Lato-Regular"]
        .into_iter()
        .filter(|face| ranges || *face == "Arabic")
        .map(|face| {
            let path = fonts.join(format!("{face}.ttf"));
            Ok(json!({"face": face, "path": path, "sha256": digest(&path)?}))
        })
        .collect::<Result<Vec<_>>>()?;
    let evidence = json!({
        "oracle": {"identity": source, "revision": revision, "command": tool, "version": version},
        "mode": if ranges { "ranges" } else { "arabic" },
        "fonts": faces, "output": output, "output_sha256": digest(&output)?,
    });
    write(&provenance, &evidence)?;
    Ok(evidence)
}

struct Sample {
    face: &'static str,
    text: &'static str,
    span: (usize, usize),
    include_span: bool,
}

fn arabic_samples() -> Vec<Sample> {
    ARABIC
        .iter()
        .map(|text| Sample {
            face: "Arabic",
            text,
            span: (0, text.len()),
            include_span: false,
        })
        .collect()
}

fn range_samples() -> Vec<Sample> {
    [
        ("Lato-Regular", "ffi", (0, 1)),
        ("Lato-Regular", "ffi", (1, 3)),
        ("Lato-Regular", "ffi", (0, 2)),
        ("Lato-Regular", "Åffi", (3, 6)),
        ("Arabic", "بلا", (0, 2)),
        ("Arabic", "بلا", (2, 4)),
        ("Arabic", "بلا", (4, 6)),
        ("Arabic", "لَا", (0, 4)),
        ("Arabic", "لَا", (4, 6)),
        ("Arabic", "العَرَبِيَّة", (0, 8)),
        ("Arabic", "بِبَبُ", (4, 8)),
    ]
    .into_iter()
    .map(|(face, text, span)| Sample {
        face,
        text,
        span,
        include_span: true,
    })
    .collect()
}

fn shape(tool: &str, fonts: &Path, sample: &Sample) -> Result<Value> {
    if !sample.text.is_char_boundary(sample.span.0) || !sample.text.is_char_boundary(sample.span.1)
    {
        return Err("oracle span does not follow UTF-8 boundaries".into());
    }
    let arabic = sample.face == "Arabic";
    let size = if arabic { 1000 } else { 2000 };
    let before = &sample.text[..sample.span.0];
    let text = &sample.text[sample.span.0..sample.span.1];
    let after = &sample.text[sample.span.1..];
    let output = Command::new(tool)
        .arg(fonts.join(format!("{}.ttf", sample.face)))
        .arg(text)
        .arg("--output-format=json")
        .arg("--no-glyph-names")
        .arg("--remove-default-ignorables")
        .arg(format!("--font-size={size}"))
        .arg(if arabic {
            "--direction=rtl"
        } else {
            "--direction=ltr"
        })
        .arg(if arabic {
            "--script=arab"
        } else {
            "--script=latn"
        })
        .arg(if arabic {
            "--language=ar"
        } else {
            "--language=en"
        })
        .arg("--features=-kern,-calt")
        .arg(format!("--text-before={before}"))
        .arg(format!("--text-after={after}"))
        .output()
        .map_err(|error| prerequisite(tool, error))?;
    if !output.status.success() {
        return Err(format!(
            "HarfBuzz oracle failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let glyphs: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    transform(sample, glyphs)
}

fn transform(sample: &Sample, glyphs: Value) -> Result<Value> {
    let glyphs = glyphs
        .as_array()
        .ok_or("HarfBuzz output is not a glyph array")?;
    let mut pen = 0i64;
    let mut positions = Vec::new();
    for glyph in glyphs {
        let id = glyph["g"]
            .as_u64()
            .or_else(|| glyph["g"].as_str()?.strip_prefix("gid")?.parse().ok())
            .ok_or("HarfBuzz glyph has no numeric id")?;
        let x = signed(glyph, "dx")?;
        let y = signed(glyph, "dy")?;
        positions.push(json!([id, pen + x, -y]));
        pen += signed(glyph, "ax")?;
    }
    let mut result = json!({"text": sample.text, "glyphs": positions, "advance": pen});
    if sample.include_span {
        result["face"] = json!(sample.face);
        result["span"] = json!([sample.span.0, sample.span.1]);
    }
    Ok(result)
}

fn signed(value: &Value, key: &str) -> Result<i64> {
    value[key]
        .as_i64()
        .ok_or_else(|| format!("HarfBuzz glyph has no integer {key}"))
}

fn command_version(tool: &str) -> Result<String> {
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
    format!("required upstream shaping oracle {tool} is unavailable: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_harfbuzz_positions_to_fixture_contract() {
        let sample = Sample {
            face: "Arabic",
            text: "سلام",
            span: (0, "سلام".len()),
            include_span: false,
        };
        let value = transform(
            &sample,
            json!([{"g": 7, "dx": 2, "dy": -3, "ax": 10}, {"g": "gid8", "dx": 1, "dy": 0, "ax": 4}]),
        )
        .unwrap();
        assert_eq!(value["glyphs"], json!([[7, 2, 3], [8, 11, 0]]));
        assert_eq!(value["advance"], 14);
    }

    #[test]
    fn missing_oracle_is_explicit() {
        let error = command_version("plectron-missing-hb-shape").unwrap_err();
        assert!(error.contains("required upstream shaping oracle"));
    }
}
