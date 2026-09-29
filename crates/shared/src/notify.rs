use std::sync::Arc;

#[derive(Clone)]
pub struct Notify(Arc<dyn Fn() + Send + Sync>);

impl Notify {
    pub fn new(callback: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }

    pub fn thread(thread: std::thread::Thread) -> Self {
        Self::new(move || thread.unpark())
    }

    pub fn notify(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for Notify {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Notify").finish()
    }
}
