use super::json::Value;
use super::receipt::Update;
use platform::shared::{Progress, Region};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filter {
    pub label: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prompt {
    pub title: Option<String>,
    pub directory: Option<String>,
    pub name: Option<String>,
    pub multiple: bool,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Cancelled,
    Paths(Vec<String>),
}

pub trait Dialog: Send {
    fn open(&mut self, prompt: &Prompt) -> Result<Selection, String>;
    fn save(&mut self, prompt: &Prompt) -> Result<Selection, String>;
    fn folder(&mut self, prompt: &Prompt) -> Result<Selection, String>;
}

pub trait Surface {
    fn title(&mut self, title: &str) -> Result<(), String>;
    fn drag(&mut self, regions: &[Region]) -> Result<(), String>;
    fn progress(&mut self, progress: Progress) -> Result<(), String>;
    fn close(&mut self) -> Result<(), String>;
}

pub struct Context<'a> {
    pub surface: &'a mut dyn Surface,
}

impl<'a> Context<'a> {
    pub fn new(surface: &'a mut dyn Surface) -> Self {
        Self { surface }
    }
}

pub struct Request<'a> {
    pub id: &'a str,
    pub command: &'a str,
    pub payload: &'a Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Running(Option<Update>),
    Completed(Value),
    Rejected(String),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub command: String,
    pub outcome: Outcome,
}

pub trait Addon: Send {
    fn namespace(&self) -> &str;

    fn allows(&self, command: &str) -> bool {
        command
            .strip_prefix(self.namespace())
            .is_some_and(|rest| rest.starts_with('.'))
    }

    fn execute(&mut self, request: &Request<'_>, context: &mut Context<'_>) -> Outcome;

    fn cancel(&mut self, id: &str, context: &mut Context<'_>) -> Option<Outcome> {
        let _ = (id, context);
        None
    }

    fn poll(&mut self, context: &mut Context<'_>) -> Vec<Notice> {
        let _ = context;
        Vec::new()
    }
}
