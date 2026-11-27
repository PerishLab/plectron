use crate::control::{Addon, Dialog};
use platform::shared::{Chrome, Extent};

pub struct Options {
    pub title: String,
    pub extent: Extent,
    pub renderer: String,
    pub chrome: Chrome,
    pub dialog: Option<Box<dyn Dialog>>,
    pub addons: Vec<Box<dyn Addon>>,
}

impl Options {
    pub fn new(title: impl Into<String>, width: usize, height: usize) -> Self {
        Self {
            title: title.into(),
            extent: Extent { width, height },
            renderer: "cpu".into(),
            chrome: Chrome::Native,
            dialog: None,
            addons: Vec::new(),
        }
    }

    pub fn dialog(mut self, dialog: impl Dialog + 'static) -> Self {
        self.dialog = Some(Box::new(dialog));
        self
    }

    pub fn addon(mut self, addon: impl Addon + 'static) -> Self {
        self.addons.push(Box::new(addon));
        self
    }
}
