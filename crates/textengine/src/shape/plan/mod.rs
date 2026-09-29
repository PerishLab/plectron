use super::{Issue, Settings, coverage::Coverage, filter::Filter};
use crate::font::{Problem, Source, data::Data};
use std::collections::BTreeSet;
mod compiler;
mod program;
mod storage;
pub(crate) use compiler::Compiler;
pub(in crate::shape) use program::Program;

pub(super) struct Subtable<'a> {
    pub kind: u16,
    pub filter: Filter<'a>,
    pub data: Data<'a>,
    pub coverage: Coverage<'a>,
    pub bases: Option<Result<Coverage<'a>, Problem>>,
}
pub(super) struct Plan<'a> {
    pub lookups: Vec<Vec<Subtable<'a>>>,
    pub issues: Vec<Issue>,
    pub feature: Option<[u8; 4]>,
    pool: Option<Data<'a>>,
    definition: Option<Data<'a>>,
}
impl<'a> Plan<'a> {
    pub fn new(source: &'a Source, tag: [u8; 4], settings: Settings) -> Result<Self, Problem> {
        let mut plan = Self {
            lookups: Vec::new(),
            issues: Vec::new(),
            feature: settings.feature,
            pool: None,
            definition: source.optional(b"GDEF"),
        };
        let Some(data) = source.optional(&tag) else {
            return Ok(plan);
        };
        if data.word(0)? != 1 || data.word(2)? > 1 {
            return Err(Problem::Unsupported);
        }
        if data.word(2)? == 1 && data.long(10)? != 0 {
            return Err(Problem::Unsupported);
        }
        let selected = select(data, tag, settings)?;
        let list = data.offset(8)?;
        plan.pool = Some(list);
        let mut coverage = 0;
        for index in selected {
            if index >= list.word(0)? {
                return Err(Problem::Malformed);
            }
            let lookup = list.offset(2 + usize::from(index) * 2)?;
            let subtables = plan.lookup(lookup, (tag, index))?;
            coverage += subtables
                .iter()
                .map(|table| table.coverage.len())
                .sum::<usize>();
            if coverage > 1_048_576 {
                return Err(Problem::Budget);
            }
            plan.lookups.push(subtables);
        }
        Ok(plan)
    }
    pub fn nested(&self, index: u16) -> Result<Vec<Subtable<'a>>, Problem> {
        let pool = self.pool.ok_or(Problem::Malformed)?;
        if index >= pool.word(0)? {
            return Err(Problem::Malformed);
        }
        let mut plan = Self {
            lookups: Vec::new(),
            issues: Vec::new(),
            feature: None,
            pool: self.pool,
            definition: self.definition,
        };
        let tables = plan.lookup(pool.offset(2 + usize::from(index) * 2)?, (*b"GSUB", index))?;
        if !plan.issues.is_empty() {
            return Err(Problem::Unsupported);
        }
        Ok(tables)
    }
    fn lookup(
        &mut self,
        data: Data<'a>,
        identity: ([u8; 4], u16),
    ) -> Result<Vec<Subtable<'a>>, Problem> {
        let (table, index) = identity;
        let flags = data.word(2)?;
        let count = usize::from(data.word(4)?);
        if count > 256 {
            return Err(Problem::Budget);
        }
        let mut subtables = Vec::new();
        for offset in 0..count {
            let mut kind = data.word(0)?;
            let mut subtable = data.offset(6 + offset * 2)?;
            if kind == if table == *b"GSUB" { 7 } else { 9 } {
                if subtable.word(0)? != 1 {
                    return Err(Problem::Unsupported);
                }
                kind = subtable.word(2)?;
                subtable = subtable.tail(subtable.long(4)? as usize)?;
            }
            let supported = if table == *b"GSUB" {
                matches!(kind, 1 | 2 | 4 | 5 | 6)
            } else {
                matches!(kind, 4..=6)
            };
            if flags & !30 != 0 || !supported {
                self.issues.push(Issue::Lookup {
                    table,
                    index,
                    kind,
                    flags,
                });
                return Ok(Vec::new());
            }
            subtables.push(Subtable {
                bases: (table == *b"GPOS").then(|| {
                    if subtable.word(0)? != 1 {
                        return Err(Problem::Unsupported);
                    }
                    Coverage::new(subtable.offset(4)?)
                }),
                filter: Filter::new(data, self.definition)?,
                kind,
                data: subtable,
                coverage: if table == *b"GSUB" && matches!(kind, 5 | 6) && subtable.word(0)? == 3 {
                    Coverage::default()
                } else {
                    Coverage::new(subtable.offset(2)?)?
                },
            });
        }
        Ok(subtables)
    }
}
fn tagged(data: Data<'_>, tag: [u8; 4], start: usize) -> Result<Option<Data<'_>>, Problem> {
    let count = usize::from(data.word(start)?);
    if count > 4096 {
        return Err(Problem::Budget);
    }
    for index in 0..count {
        let offset = start + 2 + index * 6;
        if data.slice(offset, 4)?.0 == tag {
            return Ok(Some(data.offset(offset + 4)?));
        }
    }
    Ok(None)
}
fn select(data: Data<'_>, tag: [u8; 4], settings: Settings) -> Result<BTreeSet<u16>, Problem> {
    let scripts = data.offset(4)?;
    let script = tagged(scripts, settings.script, 0)?.or(tagged(scripts, *b"DFLT", 0)?);
    let Some(script) = script else {
        return Ok(BTreeSet::new());
    };
    let language = settings
        .language
        .map(|tag| tagged(script, tag, 2))
        .transpose()?
        .flatten();
    let language = language.or(if script.word(0)? == 0 {
        None
    } else {
        Some(script.offset(0)?)
    });
    let Some(language) = language else {
        return Ok(BTreeSet::new());
    };
    if language.word(0)? != 0 {
        return Err(Problem::Unsupported);
    }
    let features = data.offset(6)?;
    let required = language.word(2)?;
    let count = usize::from(language.word(4)?);
    if count > 4096 {
        return Err(Problem::Budget);
    }
    let mut indices = Vec::new();
    if required != 0xffff && settings.feature.is_none() {
        indices.push(required);
    }
    for index in 0..count {
        indices.push(language.word(6 + index * 2)?);
    }
    let mut selected = BTreeSet::new();
    for index in indices {
        if index >= features.word(0)? {
            return Err(Problem::Malformed);
        }
        let offset = 2 + usize::from(index) * 6;
        let name = features.slice(offset, 4)?.0;
        let enabled = if let Some(feature) = settings.feature {
            name == feature
        } else if tag == *b"GSUB" {
            settings.ligatures && name == b"liga"
        } else {
            settings.marks && matches!(name, b"mark" | b"mkmk")
        };
        if enabled || (index == required && settings.feature.is_none()) {
            let feature = features.offset(offset + 4)?;
            if feature.word(2)? > 256 {
                return Err(Problem::Budget);
            }
            for lookup in 0..usize::from(feature.word(2)?) {
                selected.insert(feature.word(4 + lookup * 2)?);
            }
        }
    }
    if selected.len() > 256 {
        return Err(Problem::Budget);
    }
    Ok(selected)
}
