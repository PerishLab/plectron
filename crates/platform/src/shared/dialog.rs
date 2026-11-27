use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Open,
    Save,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filter {
    pub label: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub mode: Mode,
    pub title: Option<String>,
    pub directory: Option<PathBuf>,
    pub name: Option<String>,
    pub multiple: bool,
    pub filters: Vec<Filter>,
}

impl Dialog {
    pub fn open() -> Self {
        Self::new(Mode::Open)
    }

    pub fn save() -> Self {
        Self::new(Mode::Save)
    }

    pub fn folder() -> Self {
        Self::new(Mode::Folder)
    }

    fn new(mode: Mode) -> Self {
        Self {
            mode,
            title: None,
            directory: None,
            name: None,
            multiple: false,
            filters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Selected(Vec<PathBuf>),
    Canceled,
    Unsupported,
}
