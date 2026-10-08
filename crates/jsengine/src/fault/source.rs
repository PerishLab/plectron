use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    state: Arc<State>,
}

#[derive(Debug, PartialEq, Eq)]
struct State {
    identity: u64,
    text: Arc<str>,
}

#[derive(Clone)]
pub struct Weak(std::sync::Weak<State>);

impl Weak {
    pub fn upgrade(&self) -> Option<Source> {
        self.0.upgrade().map(|state| Source { state })
    }
}

impl Source {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        let identity = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("source identity space exhausted");
        Self {
            state: Arc::new(State {
                identity,
                text: text.into(),
            }),
        }
    }

    pub fn identity(&self) -> u64 {
        self.state.identity
    }

    pub fn text(&self) -> &str {
        &self.state.text
    }

    pub fn downgrade(&self) -> Weak {
        Weak(Arc::downgrade(&self.state))
    }

    pub(crate) fn site(&self, span: Range<usize>) -> Site {
        assert!(span.start <= span.end && self.text().get(span.clone()).is_some());
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
