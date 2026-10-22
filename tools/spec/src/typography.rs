use crate::Result;
use ::fixtures::Text;
use domengine::{Document, Kind, Node, Sheet, html, style::Inherited};
use serde::Serialize;
use std::ops::Range;
use textengine::{Run, Shaper};

#[derive(Debug, Serialize)]
pub struct Specimen {
    pub id: String,
    pub source: String,
    pub characters: Vec<Character>,
    pub run: Run,
}

#[derive(Debug, Serialize)]
pub struct Character {
    pub source: Range<usize>,
    pub code: String,
    pub nominal: Option<u16>,
}

pub fn run() -> Result<Vec<Specimen>> {
    let root = crate::root().join("apps/clusters/src");
    let html =
        std::fs::read_to_string(root.join("index.html")).map_err(|error| error.to_string())?;
    let css = std::fs::read_to_string(root.join("style.css")).map_err(|error| error.to_string())?;
    let document = html::parse(&html).map_err(|error| error.0)?;
    let sheet = Sheet::parse(&css).map_err(|error| error.0)?;
    let mut specimens = Vec::new();
    for node in document.walk() {
        let entry = document.entry(node).map_err(|error| error.0)?;
        if entry.attributes.get("class").map(String::as_str) == Some("sample") {
            specimens.push(observe(&document, &sheet, node)?);
        }
    }
    Ok(specimens)
}

fn observe(document: &Document, sheet: &Sheet, node: Node) -> Result<Specimen> {
    let entry = document.entry(node).map_err(|error| error.0)?;
    let id = entry
        .attributes
        .get("id")
        .ok_or("specimen needs identity")?;
    let mut source = String::new();
    for child in &entry.children {
        match &document.entry(*child).map_err(|error| error.0)?.kind {
            Kind::Text(text) => source.push_str(text),
            _ => return Err("specimen expects direct source text".into()),
        }
    }
    let mut inherited = Inherited::default();
    for ancestor in document.path(node).map_err(|error| error.0)?.iter().rev() {
        let style = sheet
            .compute(document, *ancestor, inherited)
            .map_err(|error| error.0)?;
        inherited = Inherited::from(&style);
    }
    let typography = inherited.typography;
    let run = Text
        .shape(&source, &typography.font, typography.spacing)
        .map_err(|error| error.0)?;
    let mut characters = Vec::new();
    for (start, character) in source.char_indices() {
        let font = run
            .spans
            .iter()
            .find(|span| span.source.contains(&start))
            .ok_or("character has no font")?;
        characters.push(Character {
            source: start..start + character.len_utf8(),
            code: format!("U+{:04X}", character as u32),
            nominal: font
                .font
                .source()
                .ok_or("specimen needs a vector face")?
                .index(character)
                .map_err(|error| format!("{error:?}"))?,
        });
    }
    Ok(Specimen {
        id: id.clone(),
        source,
        characters,
        run,
    })
}
