use super::{Record, Section};
use crate::Fault;
use textengine::{Prepared, Shaper};

impl Section {
    pub(super) fn normalize(
        source: &str,
        sections: &[Self],
        atoms: &[std::ops::Range<usize>],
    ) -> Vec<Self> {
        let mut result: Vec<Self> = Vec::new();
        let mut ranges = Vec::new();
        for range in segments(source.len(), atoms) {
            let (start, end) = (range.start, range.end);
            ranges.extend(
                textengine::unicode::graphemes(&source[start..end])
                    .into_iter()
                    .map(|range| start + range.start..start + range.end),
            );
        }
        for range in ranges {
            let index = sections.partition_point(|section| section.source.end <= range.start);
            let font = &sections[index].font;
            if let Some(previous) = result
                .last_mut()
                .filter(|previous| previous.font == *font && previous.source.end == range.start)
            {
                previous.source.end = range.end;
            } else {
                result.push(Self {
                    source: range,
                    font: font.clone(),
                });
            }
        }
        result
    }
}
impl Record {
    pub(in crate::layout) fn prepare(
        &self,
        shaper: &impl Shaper,
        sizes: Vec<[usize; 2]>,
        trace: &super::super::cache::Trace,
    ) -> Result<super::super::text::paragraph::Source, Fault> {
        let source = self.key.text.as_ref().expect("text input");
        let atoms = self
            .inline
            .as_ref()
            .map_or(&[][..], |inline| inline.atoms.as_slice());
        let mut parts = Vec::new();
        for range in segments(source.len(), atoms) {
            let (start, end) = (range.start, range.end);
            if start < end {
                trace.prepare();
                parts.push((start..end, self.segment(shaper, start..end)?));
            }
        }
        if parts.is_empty() {
            trace.prepare();
            parts.push((
                0..0,
                shaper.prepare(
                    "",
                    &self.key.style.typography.font,
                    self.key.style.typography.spacing,
                )?,
            ));
        }
        Ok(super::super::text::paragraph::Source::new(
            source.clone(),
            parts,
            atoms.iter().cloned().zip(sizes).collect(),
        ))
    }
    fn segment(
        &self,
        shaper: &impl Shaper,
        range: std::ops::Range<usize>,
    ) -> Result<Prepared, Fault> {
        let source = self.key.text.as_ref().unwrap();
        let style = &self.key.style.typography;
        let text = &source[range.clone()];
        if self.key.fonts.is_empty() {
            return Ok(shaper.prepare(text, &style.font, style.spacing)?);
        }
        let mut selected = Vec::new();
        let index = self
            .key
            .fonts
            .partition_point(|section| section.source.end <= range.start);
        for section in self.key.fonts[index..]
            .iter()
            .take_while(|section| section.source.start < range.end)
        {
            let start = section.source.start.max(range.start);
            let end = section.source.end.min(range.end);
            let mut spans = shaper.select(&source[start..end], &section.font, style.spacing)?;
            for span in &mut spans {
                span.source.start += start - range.start;
                span.source.end += start - range.start;
            }
            selected.extend(spans);
        }
        Ok(Prepared::new(text, selected)?)
    }
}

fn segments(
    length: usize,
    atoms: &[std::ops::Range<usize>],
) -> impl Iterator<Item = std::ops::Range<usize>> {
    std::iter::once(0)
        .chain(atoms.iter().map(|atom| atom.end))
        .zip(
            atoms
                .iter()
                .map(|atom| atom.start)
                .chain(std::iter::once(length)),
        )
        .map(|(start, end)| start..end)
}
