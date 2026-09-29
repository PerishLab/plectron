use super::Resource;
use std::collections::HashSet;

const CAPACITY: usize = 4;

#[derive(Debug, Clone)]
pub(super) enum Resources {
    Inline([Option<Resource>; CAPACITY]),
    Indexed(HashSet<Resource, ahash::RandomState>),
}

impl Default for Resources {
    fn default() -> Self {
        Self::Inline([const { None }; CAPACITY])
    }
}

impl Resources {
    pub fn insert(&mut self, resource: Resource) {
        let values = match self {
            Self::Inline(values) => values,
            Self::Indexed(values) => {
                values.insert(resource);
                return;
            }
        };
        if values.iter().flatten().any(|value| value == &resource) {
            return;
        }
        if let Some(slot) = values.iter_mut().find(|value| value.is_none()) {
            *slot = Some(resource);
            return;
        }
        let mut indexed: HashSet<_, ahash::RandomState> =
            values.iter().flatten().cloned().collect();
        indexed.insert(resource);
        *self = Self::Indexed(indexed);
    }
    pub fn extend(&mut self, resources: impl IntoIterator<Item = Resource>) {
        for resource in resources {
            self.insert(resource);
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &Resource> {
        let (inline, indexed) = match self {
            Self::Inline(values) => (Some(values), None),
            Self::Indexed(values) => (None, Some(values)),
        };
        inline
            .into_iter()
            .flatten()
            .filter_map(Option::as_ref)
            .chain(indexed.into_iter().flatten())
    }
}
