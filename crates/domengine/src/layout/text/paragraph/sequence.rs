mod parts;
use graphics::shared::Scalar;
use parts::{Part, Parts};
use std::ops::Range;
use std::sync::Arc;
use textengine::{Cluster, Run};

#[derive(Clone, Copy, PartialEq)]
pub(in crate::layout) enum Item {
    Text { part: usize, cluster: usize },
    Atom { index: usize, size: [usize; 2] },
}
#[derive(Clone, PartialEq)]
pub(in crate::layout) struct Unit {
    pub source: Range<usize>,
    pub advance: Scalar,
    pub item: Item,
}
#[derive(Clone)]
pub(in crate::layout) struct Sequence {
    pub parts: Arc<Parts>,
    pub units: Arc<Vec<Unit>>,
    pub height: Scalar,
    pub ascent: Scalar,
    pub issues: Vec<textengine::Issue>,
    pub length: usize,
}
impl Sequence {
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.parts, &other.parts)
            && (self.height, self.ascent, self.length) == (other.height, other.ascent, other.length)
            && (&self.issues, &self.units) == (&other.issues, &other.units)
    }
    pub fn new(length: usize) -> Self {
        Self {
            parts: Arc::default(),
            units: Arc::default(),
            height: Scalar::ZERO,
            ascent: Scalar::ZERO,
            issues: Vec::new(),
            length,
        }
    }
    pub fn append(&mut self, source: Range<usize>, mut run: Run) {
        let descent = (self.height - self.ascent).max(run.height - run.ascent);
        self.ascent = self.ascent.max(run.ascent);
        self.height = self.ascent + descent;
        for issue in &mut run.issues {
            match issue {
                textengine::Issue::Unmapped(range) | textengine::Issue::Unpositioned(range) => {
                    range.start += source.start;
                    range.end += source.start;
                }
                textengine::Issue::Lookup { .. } => {}
            }
        }
        self.issues.append(&mut run.issues);
        let units = Arc::make_mut(&mut self.units);
        for (cluster, value) in run.clusters.iter().enumerate() {
            units.push(Unit {
                source: source.start + value.source.start..source.start + value.source.end,
                advance: value.advance,
                item: Item::Text {
                    part: self.parts.len(),
                    cluster,
                },
            });
        }
        Arc::make_mut(&mut self.parts).push(run);
    }
    pub fn advance(&mut self, index: usize, advance: Scalar) {
        if self.units[index].advance != advance {
            Arc::make_mut(&mut self.units)[index].advance = advance;
        }
    }
    pub fn text(&self, index: usize) -> Option<(&Part, &Cluster)> {
        if let Item::Text { part, cluster } = self.units[index].item {
            let part = &self.parts[part];
            Some((part, &part.run.clusters[cluster]))
        } else {
            None
        }
    }
    pub fn atom(&self, index: usize) -> Option<(usize, [usize; 2])> {
        match self.units[index].item {
            Item::Atom { index, size } => Some((index, size)),
            Item::Text { .. } => None,
        }
    }
    pub fn cost(&self) -> shared::cache::Cost {
        let mut cost = shared::cache::Cost::new(
            std::mem::size_of::<Self>()
                + std::mem::size_of::<Vec<Unit>>()
                + self.units.len() * std::mem::size_of::<Unit>()
                + self.issues.len() * std::mem::size_of::<textengine::Issue>(),
        );
        cost.extend(self.parts.cost());
        cost
    }
}
