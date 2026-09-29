use super::{Kind, sequence::Sequence};
use crate::style::Typography;
use graphics::shared::Scalar;

pub(in crate::layout::text) fn kinds(
    sequence: &mut Sequence,
    source: &str,
    style: &Typography,
) -> Vec<Kind> {
    let mut kinds = Vec::with_capacity(sequence.units.len());
    let mut previous = false;
    for cluster in std::sync::Arc::make_mut(&mut sequence.units) {
        let text = &source[cluster.source.clone()];
        let space = text
            .chars()
            .all(|character| matches!(character, ' ' | '\t' | '\r' | '\n' | '\u{000c}'));
        let kind = if style.whitespace.collapse() && space {
            if previous {
                cluster.advance = Scalar::ZERO;
            }
            Kind::Space
        } else {
            match text {
                text if text.chars().all(textengine::breaking::mandatory) => Kind::Break,
                "\t" => Kind::Tab,
                " " => Kind::Space,
                _ => Kind::Text,
            }
        };
        if kind == Kind::Break {
            cluster.advance = Scalar::ZERO;
        }
        previous = space;
        kinds.push(kind);
    }
    kinds
}
