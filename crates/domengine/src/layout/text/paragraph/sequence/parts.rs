use shared::cache::Cost;
use std::{ops::Deref, sync::OnceLock};
use textengine::{Cluster, Run};

#[derive(Clone)]
pub(in crate::layout) struct Part {
    pub run: Run,
}

#[derive(Clone, Default)]
pub(in crate::layout) struct Parts {
    values: Vec<Part>,
    accounting: OnceLock<Cost>,
}

impl Parts {
    pub fn push(&mut self, run: Run) {
        self.accounting.take();
        self.values.push(Part { run });
    }
    pub fn cost(&self) -> &Cost {
        self.accounting.get_or_init(|| {
            let mut cost = Cost::new(std::mem::size_of::<Self>());
            for part in &self.values {
                cost.bytes += std::mem::size_of::<Part>()
                    + part.run.spans.len() * std::mem::size_of::<textengine::Span>()
                    + part.run.clusters.len() * std::mem::size_of::<Cluster>()
                    + part.run.glyphs.len() * std::mem::size_of::<textengine::Glyph>();
                for resource in part
                    .run
                    .spans
                    .iter()
                    .filter_map(|span| span.font.resource())
                {
                    cost.resource(resource);
                }
            }
            cost.share()
        })
    }
}

impl Deref for Parts {
    type Target = [Part];
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
