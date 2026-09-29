use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub command: Command,
}

impl Request {
    pub fn new(command: Command) -> Self {
        Self {
            version: 1,
            command,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Command {
    Query {
        command: super::query::Command,
    },
    Observe {
        command: super::observation::Command,
    },
    Config {
        command: config::Command,
    },
    Cache {
        unit: u64,
        id: u64,
        command: shared::cache::Command,
    },
    Status {},
    Control {},
    Windows {},
    Services {},
    Export {
        unit: u64,
    },
    Page {
        unit: u64,
    },
    Evaluate {
        unit: u64,
        source: String,
    },
    Collect {
        unit: u64,
    },
    Input {
        unit: u64,
        input: webengine::keyboard::Input,
    },
    Edit {
        unit: u64,
        edit: webengine::Edit,
    },
    Composition {
        unit: u64,
        input: webengine::ime::Input,
    },
    Pointer {
        unit: u64,
        x: usize,
        y: usize,
    },
    Scroll {
        unit: u64,
        delta: i64,
    },
    Wheel {
        unit: u64,
        x: usize,
        y: usize,
        delta: i64,
    },
    Pan {
        unit: u64,
        scene: u64,
        space: u64,
        delta: i64,
    },
    Send {
        unit: u64,
        text: String,
    },
    Hold {
        unit: Option<u64>,
    },
    Unhold {
        unit: Option<u64>,
    },
    Pause {},
    Resume {},
    Step {},
    Eval {
        source: String,
    },
    Click {
        unit: u64,
    },
    Close {
        unit: u64,
    },
    Shutdown {},
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Response<R = Reply> {
    pub version: u32,
    pub result: Result<R, String>,
}

impl Command {
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Query { .. } => "ipc.query",
            Self::Observe { .. } => "ipc.observe",
            Self::Config { .. } => "ipc.config",
            Self::Cache { .. } => "ipc.cache",
            Self::Status { .. } => "ipc.status",
            Self::Control { .. } => "ipc.control",
            Self::Windows { .. } => "ipc.windows",
            Self::Services { .. } => "ipc.services",
            Self::Export { .. } => "ipc.export",
            Self::Page { .. } => "ipc.page",
            Self::Evaluate { .. } => "ipc.evaluate",
            Self::Collect { .. } => "ipc.collect",
            Self::Input { .. } => "ipc.input",
            Self::Edit { .. } => "ipc.edit",
            Self::Composition { .. } => "ipc.composition",
            Self::Pointer { .. } => "ipc.pointer",
            Self::Scroll { .. } => "ipc.scroll",
            Self::Wheel { .. } => "ipc.wheel",
            Self::Pan { .. } => "ipc.pan",
            Self::Send { .. } => "ipc.send",
            Self::Hold { .. } => "ipc.hold",
            Self::Unhold { .. } => "ipc.unhold",
            Self::Pause { .. } => "ipc.pause",
            Self::Resume { .. } => "ipc.resume",
            Self::Step { .. } => "ipc.step",
            Self::Eval { .. } => "ipc.eval",
            Self::Click { .. } => "ipc.click",
            Self::Close { .. } => "ipc.close",
            Self::Shutdown { .. } => "ipc.shutdown",
        }
    }
}

impl<R> Response<R> {
    pub fn error(error: impl ToString) -> Self {
        Self {
            version: 1,
            result: Err(error.to_string()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Reply {
    Query {
        reply: super::query::Reply,
    },
    Observing {
        status: Option<observe::Status>,
    },
    Observation {
        report: Box<observe::Report>,
    },
    Config {
        entries: config::Entries,
    },
    Control {
        snapshot: crate::control::Snapshot,
    },
    Status {
        pid: u32,
        mode: Mode,
        windows: usize,
        queued: usize,
        dropped: u64,
        fault: Option<String>,
    },
    Windows {
        entries: Vec<Window>,
    },
    Evaluation {
        display: String,
    },
    Page {
        snapshot: Box<webengine::Snapshot>,
    },
    Services {
        entries: Vec<crate::resource::Service>,
    },
    Accepted {
        job: u64,
        record: Option<u64>,
    },
    Ack,
    Stepped {
        dispatched: bool,
    },
    Shutdown {
        errors: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Running,
    Paused,
    Stopping,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Window {
    pub id: u64,
    pub title: String,
    pub width: usize,
    pub height: usize,
}
