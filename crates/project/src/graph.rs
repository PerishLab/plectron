use crate::{Kind, Module, Origin, Span};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Module,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identity(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub target: Module,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub module: Module,
    pub kind: Kind,
    pub content: Identity,
    pub dependencies: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    pub identity: Identity,
    pub entry: Module,
    pub order: Vec<Module>,
    pub modules: BTreeMap<Module, Node>,
}

impl Graph {
    pub fn binding(&self, module: &Module) -> String {
        if module == &self.entry {
            return module.binding();
        }
        format!("{}__{}", self.entry.binding(), module.binding())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cache {
    pub hits: usize,
    pub misses: usize,
    pub entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub javascript: String,
    pub graph: Graph,
    pub cache: Cache,
    pub origins: Vec<Origin>,
}

impl Output {
    pub fn origin(&self, span: Span) -> Option<Origin> {
        self.origins
            .iter()
            .find(|origin| origin.generated.start <= span.start && span.end <= origin.generated.end)
            .map(|origin| origin.project(span))
    }
}
