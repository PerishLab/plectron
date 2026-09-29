use std::str::{self, Utf8Error};

#[derive(Debug, Clone, Copy)]
pub struct Resource {
    bytes: &'static [u8],
}

impl Resource {
    pub const fn new(bytes: &'static [u8]) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &'static [u8] {
        self.bytes
    }

    pub fn text(&self) -> Result<&'static str, Utf8Error> {
        str::from_utf8(self.bytes)
    }
}
