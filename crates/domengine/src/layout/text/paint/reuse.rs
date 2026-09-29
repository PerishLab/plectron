use crate::Content;
use crate::layout::{fragment::builder::Builder, measure::View};
use std::sync::Arc;

pub(super) enum Change {
    Binding(Content),
}

impl Builder<'_> {
    pub(super) fn change(&self, measured: &View<'_>) -> Option<Change> {
        if !self.projection.materialization {
            return None;
        }
        let lines = measured.text.as_ref()?;
        if lines.ranges.len() != 1 {
            return None;
        }
        let sequence = &lines.ranges[0].sequence;
        if sequence.parts.len() != 1 || sequence.parts[0].run.spans.len() != 1 {
            return None;
        }
        let previous = self.previous.as_ref()?;
        let matching = previous
            .bindings
            .iter()
            .any(|(node, old)| *node == measured.material().node() && Arc::ptr_eq(old, lines));
        if !matching {
            return None;
        }
        previous.fragments.iter().find_map(|fragment| {
            if fragment.local.node != Some(measured.material().node().id()) {
                return None;
            }
            if let Content::Text {
                font,
                glyphs,
                clusters,
                ..
            } = &fragment.local.content
            {
                Some(Change::Binding(Content::Text {
                    font: font.clone(),
                    glyphs: glyphs.clone(),
                    clusters: clusters.clone(),
                    clip: None,
                    color: measured.material().style().color,
                }))
            } else {
                None
            }
        })
    }
}
