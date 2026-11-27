use crate::Fault;
use std::sync::{Arc, Mutex};

pub trait Port: Send + Sync {
    fn read(&self) -> Result<Option<String>, Fault>;
    fn write(&self, text: &str) -> Result<(), Fault>;
}

#[derive(Default)]
pub struct Memory {
    value: Mutex<Option<String>>,
}

impl Memory {
    pub fn text(&self) -> Option<String> {
        self.value.lock().expect("clipboard lock").clone()
    }

    pub fn put(&self, text: &str) {
        *self.value.lock().expect("clipboard lock") = Some(text.into());
    }
}

impl Port for Memory {
    fn read(&self) -> Result<Option<String>, Fault> {
        Ok(self.text())
    }

    fn write(&self, text: &str) -> Result<(), Fault> {
        self.put(text);
        Ok(())
    }
}

#[derive(Clone)]
pub struct Clipboard {
    port: Arc<dyn Port>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new(Arc::new(Memory::default()))
    }
}

impl Clipboard {
    pub fn new(port: Arc<dyn Port>) -> Self {
        Self { port }
    }

    pub fn bind(&mut self, port: Arc<dyn Port>) {
        self.port = port;
    }

    pub fn read(&self) -> Result<Option<String>, Fault> {
        self.port.read()
    }

    pub fn write(&self, text: &str) -> Result<(), Fault> {
        self.port.write(text)
    }
}
