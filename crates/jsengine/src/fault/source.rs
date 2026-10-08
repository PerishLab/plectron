use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    identity: u64,
    text: Arc<str>,
}

impl Source {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        let identity = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("source identity space exhausted");
        Self {
            identity,
            text: text.into(),
        }
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn site(&self, span: Range<usize>) -> Site {
        assert!(span.start <= span.end && self.text.get(span.clone()).is_some());
        Site {
            source: self.clone(),
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub source: Source,
    pub span: Range<usize>,
}
