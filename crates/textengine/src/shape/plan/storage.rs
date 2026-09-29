use super::{Plan, Subtable};
use crate::font::{Problem, data::Data};
use crate::shape::{Issue, buffer::Buffer, coverage, filter};
use std::ops::Range;

pub(super) struct Storage {
    lookups: Vec<Vec<Record>>,
    issues: Vec<Issue>,
    feature: Option<[u8; 4]>,
    pool: Option<Range<usize>>,
    definition: Option<Range<usize>>,
}
struct Record {
    kind: u16,
    filter: filter::Record,
    data: Range<usize>,
    coverage: coverage::Record,
    bases: Option<Result<coverage::Record, Problem>>,
}
impl Storage {
    pub fn new(plan: Plan<'_>, source: &[u8]) -> Self {
        Self {
            lookups: plan
                .lookups
                .into_iter()
                .map(|lookup| {
                    lookup
                        .into_iter()
                        .map(|table| Record {
                            kind: table.kind,
                            filter: table.filter.record(source),
                            data: table.data.range(source),
                            coverage: table.coverage.record(source),
                            bases: table
                                .bases
                                .map(|bases| bases.map(|bases| bases.record(source))),
                        })
                        .collect()
                })
                .collect(),
            issues: plan.issues,
            feature: plan.feature,
            pool: plan.pool.map(|data| data.range(source)),
            definition: plan.definition.map(|data| data.range(source)),
        }
    }
    pub fn view<'a>(&self, source: &'a [u8]) -> Plan<'a> {
        Plan {
            lookups: self
                .lookups
                .iter()
                .map(|lookup| {
                    lookup
                        .iter()
                        .map(|record| Subtable {
                            kind: record.kind,
                            filter: record.filter.view(source),
                            data: Data(&source[record.data.clone()]),
                            coverage: record.coverage.view(source),
                            bases: record.bases.as_ref().map(|bases| match bases {
                                Ok(bases) => Ok(bases.view(source)),
                                Err(problem) => Err(*problem),
                            }),
                        })
                        .collect()
                })
                .collect(),
            issues: self.issues.clone(),
            feature: self.feature,
            pool: self.pool.as_ref().map(|range| Data(&source[range.clone()])),
            definition: self
                .definition
                .as_ref()
                .map(|range| Data(&source[range.clone()])),
        }
    }
    pub fn admits(
        &self,
        source: &[u8],
        buffer: &Buffer,
        positioning: bool,
    ) -> Result<bool, Problem> {
        for record in self.lookups.iter().flatten() {
            let coverage = record.coverage.view(source);
            if coverage.len() == 0
                || buffer
                    .slots
                    .iter()
                    .any(|slot| coverage.index(slot.glyph.id).is_some())
            {
                return Ok(true);
            }
            if positioning && record.kind == 6 {
                let bases = record.bases.as_ref().ok_or(Problem::Malformed)?;
                let bases = bases.as_ref().map_err(|problem| *problem)?.view(source);
                if buffer
                    .slots
                    .iter()
                    .any(|slot| bases.index(slot.glyph.id).is_some())
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }
    pub fn bytes(&self) -> usize {
        self.lookups.capacity() * std::mem::size_of::<Vec<Record>>()
            + self
                .lookups
                .iter()
                .map(|lookup| lookup.capacity() * std::mem::size_of::<Record>())
                .sum::<usize>()
            + self.issues.capacity() * std::mem::size_of::<Issue>()
    }
}
