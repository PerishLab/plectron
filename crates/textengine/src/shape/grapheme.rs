use super::{Shaped, buffer::View};

pub(super) fn merge(shaped: &mut Shaped, source: View<'_>, spacing: usize) {
    shaped.clusters.dedup_by(|cluster, previous| {
        if source.contains(cluster.source.start) {
            return false;
        }
        let tracking =
            usize::from(!previous.glyphs.is_empty() && !cluster.glyphs.is_empty()) * spacing;
        previous.source.end = cluster.source.end;
        previous.glyphs.end = cluster.glyphs.end;
        previous.advance = previous.advance + cluster.advance - (tracking as i64).into();
        true
    });
}
