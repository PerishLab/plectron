use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Budget {
    limit: usize,
    used: AtomicUsize,
}
impl Budget {
    pub const fn new(limit: usize) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }
    pub fn acquire(&self) -> Option<Permit<'_>> {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < self.limit).then(|| count + 1)
            })
            .ok()
            .map(|_| Permit { budget: self })
    }
}
#[must_use]
pub struct Permit<'a> {
    budget: &'a Budget,
}
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(1, Ordering::AcqRel);
    }
}
