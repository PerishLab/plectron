use super::paragraph::{Kind, Paragraph};
use graphics::shared::Scalar;

#[derive(Clone, Copy)]
pub(in crate::layout) enum Change {
    Width {
        before: Option<usize>,
        after: Option<usize>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::layout) enum Plan {
    Retain,
    Layout,
}

#[derive(Clone, Copy)]
pub(in crate::layout) struct Rule {
    minimum: usize,
}

impl Rule {
    pub fn select(self, change: Change) -> Plan {
        let Change::Width { before, after } = change;
        if [before, after].into_iter().all(|width| self.accepts(width)) {
            Plan::Retain
        } else {
            Plan::Layout
        }
    }
    pub fn accepts(self, width: Option<usize>) -> bool {
        width.is_none_or(|width| width >= self.minimum)
    }
}

impl Paragraph {
    pub(in crate::layout) fn rule(&self) -> Option<Rule> {
        if self.refines()
            || self
                .kinds
                .iter()
                .any(|kind| matches!(kind, Kind::Tab | Kind::Break))
        {
            return None;
        }
        let width = self
            .sequence
            .units
            .iter()
            .fold(Scalar::ZERO, |width, unit| {
                width + unit.advance.max(Scalar::ZERO)
            });
        Some(Rule {
            minimum: width.ceil(),
        })
    }
}
