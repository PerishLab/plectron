use super::Source;
use crate::Fault;
use serde::{Deserialize, Serialize};
use std::{
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct Asset {
    resource: shared::cache::Resource,
    id: u64,
    source: Source,
}
#[derive(Debug, Clone)]
pub struct Lease(Arc<Asset>);
impl Lease {
    pub fn new(bytes: Arc<[u8]>) -> Result<Self, Fault> {
        let source = Source::parse(bytes)?;
        let id = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Fault("font identities exhausted".into()))?;
        Ok(Self(Arc::new(Asset {
            resource: shared::cache::Resource::new(source.bytes().len()),
            id,
            source,
        })))
    }
    pub fn resource(&self) -> shared::cache::Resource {
        self.0.resource.clone()
    }
    pub fn id(&self) -> u64 {
        self.0.id
    }
    pub fn length(&self) -> usize {
        self.source().bytes().len()
    }
    pub fn source(&self) -> &Source {
        &self.0.source
    }
}
impl PartialEq for Lease {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Eq for Lease {}
impl Hash for Lease {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}
impl Serialize for Lease {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.source().bytes().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Lease {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        Self::new(Arc::from(bytes)).map_err(serde::de::Error::custom)
    }
}
