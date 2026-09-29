use crate::{Fault, Font, Selection};
use std::ops::Range;

pub(super) fn select(
    source: &str,
    fonts: &[Font],
    spacing: usize,
    selector: &mut super::selection::Selector,
) -> Result<Vec<Selection>, Fault> {
    if source.len() > 65_536 || spacing > 4096 {
        return Err(Fault("text budget exhausted".into()));
    }
    let first = fonts
        .first()
        .ok_or_else(|| Fault("no registered font candidate".into()))?;
    if fonts.len() == 1 {
        return Ok(vec![Selection {
            source: 0..source.len(),
            font: first.clone(),
            spacing,
        }]);
    }
    let mut segments: Vec<(Range<usize>, usize)> = Vec::new();
    let mut selections = std::collections::BTreeMap::new();
    let mut assigned = Vec::new();
    for range in crate::unicode::graphemes(source) {
        let text = &source[range.clone()];
        let index = if text.chars().all(crate::unicode::hidden) {
            None
        } else if let Some(index) = selections.get(text) {
            Some(*index)
        } else {
            let index = selector.choose(text, fonts)?;
            selections.insert(text, index);
            Some(index)
        };
        assigned.push((range, index));
    }
    let mut previous = assigned.iter().find_map(|(_, index)| *index).unwrap_or(0);
    for (range, index) in assigned {
        let index = index.unwrap_or(previous);
        previous = index;
        if let Some((previous, _)) = segments.last_mut().filter(|(_, font)| *font == index) {
            previous.end = range.end;
        } else {
            segments.push((range, index));
        }
    }
    if segments.is_empty() {
        return Ok(vec![Selection {
            source: 0..0,
            font: first.clone(),
            spacing,
        }]);
    }
    Ok(segments
        .into_iter()
        .map(|(source, index)| Selection {
            source,
            font: fonts[index].clone(),
            spacing,
        })
        .collect())
}

pub(super) fn choose(source: &str, fonts: &[Font]) -> Result<usize, Fault> {
    let mut covered = None;
    for (index, font) in fonts.iter().enumerate() {
        if !coverage(source, font)? {
            continue;
        }
        covered.get_or_insert(index);
        let shaped = font.shape(source, 0)?;
        if shaped.issues.is_empty() {
            return Ok(index);
        }
    }
    Ok(covered.unwrap_or(0))
}
fn coverage(source: &str, font: &Font) -> Result<bool, Fault> {
    for character in source.chars() {
        if crate::unicode::hidden(character) {
            continue;
        }
        let character = if character.is_ascii_whitespace() {
            ' '
        } else {
            character
        };
        let covered = match font.source() {
            Some(face) => face.index(character)?.is_some(),
            None => character.is_ascii(),
        };
        if !covered {
            return Ok(false);
        }
    }
    Ok(true)
}
