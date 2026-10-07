use oxc_span::Span;

use super::{Lowered, Repair};

pub(super) struct Buffer<'a> {
    original: &'a str,
    bytes: Vec<u8>,
    removed: Vec<bool>,
    retired: Vec<Span>,
    repairs: Vec<Repair>,
}

impl<'a> Buffer<'a> {
    pub fn new(original: &'a str) -> Self {
        Self {
            original,
            bytes: original.as_bytes().to_vec(),
            removed: vec![false; original.len()],
            retired: Vec::new(),
            repairs: Vec::new(),
        }
    }

    pub fn original(&self, span: Span) -> &str {
        &self.original[span.start as usize..span.end as usize]
    }

    pub fn erase(&mut self, span: Span) {
        let start = span.start as usize;
        let end = span.end as usize;
        self.removed[start..end].fill(true);
        for (offset, character) in self.original[start..end].char_indices() {
            if !matches!(character, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
                let range = start + offset..start + offset + character.len_utf8();
                self.bytes[range].fill(b' ');
            }
        }
    }

    pub fn retire(&mut self, span: Span) {
        self.erase(span);
        self.write(span.start, b';', span);
        self.retired.push(span);
    }

    pub fn retired(&self, span: Span) -> bool {
        self.retired
            .iter()
            .any(|outer| outer.start <= span.start && span.end <= outer.end)
    }

    pub fn removed(&self, span: Span) -> bool {
        self.removed[span.start as usize..span.end as usize]
            .iter()
            .all(|removed| *removed)
    }

    pub fn write(&mut self, position: u32, byte: u8, source: Span) {
        self.bytes[position as usize] = byte;
        self.repairs.push(Repair {
            generated: super::super::diagnostic::span(Span::new(position, position + 1)),
            source: super::super::diagnostic::span(source),
        });
    }

    pub fn finish(self) -> Lowered {
        Lowered {
            text: String::from_utf8(self.bytes).expect("erasure preserves UTF-8 boundaries"),
            repairs: self.repairs,
        }
    }
}
