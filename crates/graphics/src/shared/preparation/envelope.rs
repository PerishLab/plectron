use crate::shared::{Clip, Glyph, Output};

pub(super) const LIMIT: u64 = 1_048_576;

#[derive(Clone, Copy)]
pub(super) struct Envelope(Option<[f64; 4]>);
impl Envelope {
    pub fn rect(rect: crate::shared::Rect, output: Output) -> Option<Self> {
        if rect.x.saturating_add(rect.width) > LIMIT as usize
            || rect.y.saturating_add(rect.height) > LIMIT as usize
        {
            return None;
        }
        if rect.width == 0 || rect.height == 0 {
            return Some(Self::new());
        }
        Some(Self(Some(
            [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
                .map(|v| v as f64 * output.scale()),
        )))
    }
    pub fn extend(&mut self, other: Self) {
        if let Some(bounds) = other.0 {
            self.include(bounds);
        }
    }
    pub fn shifted(self, shift: [i64; 2], output: Output) -> Option<Self> {
        let Some(bounds) = self.0 else {
            return Some(self);
        };
        let shift = shift.map(|v| v as f64 * output.scale());
        let bounds = std::array::from_fn(|index| bounds[index] + shift[index % 2]);
        bounds
            .iter()
            .all(|v: &f64| v.is_finite() && v.abs() < super::placement::LIMIT - 2.0)
            .then_some(Self(Some(bounds)))
    }
    pub fn new() -> Self {
        Self(None)
    }
    pub fn include(&mut self, bounds: [f64; 4]) {
        if bounds[0] > bounds[2] || bounds[1] > bounds[3] {
            return;
        }
        self.0 = Some(self.0.map_or(bounds, |old| {
            [
                old[0].min(bounds[0]),
                old[1].min(bounds[1]),
                old[2].max(bounds[2]),
                old[3].max(bounds[3]),
            ]
        }));
    }
    pub fn excludes(&self, shift: [i64; 2], output: Output, clip: Clip) -> bool {
        let Some(bounds) = self.0 else {
            return true;
        };
        let shift = shift.map(|value| value as f64 * output.scale());
        let bounds = [
            bounds[0] + shift[0],
            bounds[1] + shift[1],
            bounds[2] + shift[0],
            bounds[3] + shift[1],
        ];
        if bounds
            .iter()
            .any(|value| value.abs() > super::placement::LIMIT - 2.0)
        {
            return false;
        }
        let edges = clip.edges().map(f64::from);
        (0..2).any(|axis| {
            bounds[axis].floor() - 1.0 >= edges[axis + 2]
                || bounds[axis + 2].ceil() + 1.0 <= edges[axis]
        })
    }
}

pub(super) fn bounded(glyphs: &[Glyph]) -> bool {
    glyphs
        .iter()
        .all(|glyph| glyph.x.value().abs() <= LIMIT as f64 && glyph.y.value().abs() <= LIMIT as f64)
}
