use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tone {
    Normal,
    Bold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub family: String,
    pub tone: Tone,
    pub bytes: Arc<[u8]>,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    Found(Asset),
    Absent,
    Unsupported,
}

pub trait Fonts<E>: Send + Sync {
    fn discover(&self, family: &str, tone: Tone) -> Result<Discovery, E>;
    fn defaults(&self) -> &'static [&'static str] {
        &[]
    }
}
