use super::addon::{Addon, Context, Dialog, Filter, Outcome, Prompt, Request, Selection};
use super::json::Value;
use platform::shared::{Mode, Progress, Region};
use std::collections::BTreeMap;

const TITLE: usize = 512;
const REGIONS: usize = 64;
const FILTERS: usize = 32;
const EXTENSIONS: usize = 32;

pub struct Host {
    dialog: Option<Box<dyn Dialog>>,
}

impl Host {
    pub fn new(dialog: Option<Box<dyn Dialog>>) -> Self {
        Self { dialog }
    }

    fn title(&self, payload: &Value, context: &mut Context<'_>) -> Outcome {
        let Some(text) = payload.field("title").and_then(Value::text) else {
            return Outcome::Rejected("window.title requires a string title".into());
        };
        if text.len() > TITLE || text.chars().any(char::is_control) {
            return Outcome::Rejected("window.title is invalid or exceeds budget".into());
        }
        match context.surface.title(text) {
            Ok(()) => Outcome::Completed(Value::Null),
            Err(error) => Outcome::Rejected(error),
        }
    }

    fn drag(&self, payload: &Value, context: &mut Context<'_>) -> Outcome {
        let Some(items) = payload.field("regions").and_then(Value::array) else {
            return Outcome::Rejected("window.drag requires a regions array".into());
        };
        if items.len() > REGIONS {
            return Outcome::Rejected("window.drag exceeds region budget".into());
        }
        let mut regions = Vec::with_capacity(items.len());
        for item in items {
            match Fields(item).region() {
                Ok(parsed) => regions.push(parsed),
                Err(error) => return Outcome::Rejected(error),
            }
        }
        match context.surface.drag(&regions) {
            Ok(()) => Outcome::Completed(Value::Null),
            Err(error) => Outcome::Rejected(error),
        }
    }

    fn progress(&self, payload: &Value, context: &mut Context<'_>) -> Outcome {
        let parsed = match Fields(payload).progress() {
            Ok(value) => value,
            Err(error) => return Outcome::Rejected(error),
        };
        match context.surface.progress(parsed) {
            Ok(()) => Outcome::Completed(Value::Null),
            Err(error) => Outcome::Rejected(error),
        }
    }

    fn close(&self, payload: &Value, context: &mut Context<'_>) -> Outcome {
        let valid = match payload {
            Value::Null => true,
            Value::Object(map) => map.is_empty(),
            _ => false,
        };
        if !valid {
            return Outcome::Rejected("window.close expects null or an empty object".into());
        }
        match context.surface.close() {
            Ok(()) => Outcome::Completed(Value::Null),
            Err(error) => Outcome::Rejected(error),
        }
    }

    fn dialog(&mut self, command: &str, payload: &Value, context: &mut Context<'_>) -> Outcome {
        let parsed = match Fields(payload).prompt() {
            Ok(value) => value,
            Err(error) => return Outcome::Rejected(error),
        };
        let mode = match command {
            "dialog.openFile" => Mode::Open,
            "dialog.saveFile" => Mode::Save,
            "dialog.pickFolder" => Mode::Folder,
            _ => return Outcome::Rejected("unsupported dialog command".into()),
        };
        let result = if let Some(port) = self.dialog.as_deref_mut() {
            match mode {
                Mode::Open => port.open(&parsed),
                Mode::Save => port.save(&parsed),
                Mode::Folder => port.folder(&parsed),
            }
        } else {
            match context.surface.dialog(mode, &parsed) {
                Ok(Some(selection)) => Ok(selection),
                Ok(None) => Err("dialog port is unavailable".into()),
                Err(error) => Err(error),
            }
        };
        match result {
            Ok(selection) => Outcome::Completed(encode(selection)),
            Err(error) => Outcome::Rejected(error),
        }
    }
}

impl Addon for Host {
    fn namespace(&self) -> &str {
        "host"
    }

    fn allows(&self, command: &str) -> bool {
        matches!(
            command,
            "window.title"
                | "window.drag"
                | "window.progress"
                | "window.close"
                | "dialog.openFile"
                | "dialog.saveFile"
                | "dialog.pickFolder"
        )
    }

    fn execute(&mut self, request: &Request<'_>, context: &mut Context<'_>) -> Outcome {
        match request.command {
            "window.title" => self.title(request.payload, context),
            "window.drag" => self.drag(request.payload, context),
            "window.progress" => self.progress(request.payload, context),
            "window.close" => self.close(request.payload, context),
            "dialog.openFile" | "dialog.saveFile" | "dialog.pickFolder" => {
                self.dialog(request.command, request.payload, context)
            }
            _ => Outcome::Rejected("unauthorized host command".into()),
        }
    }
}

struct Fields<'a>(&'a Value);

impl Fields<'_> {
    fn region(&self) -> Result<Region, String> {
        let left = self.coordinate("left")?;
        let top = self.coordinate("top")?;
        let right = self.coordinate("right")?;
        let bottom = self.coordinate("bottom")?;
        if left >= right || top >= bottom {
            return Err("window.drag region requires left < right and top < bottom".into());
        }
        Ok(Region {
            left,
            top,
            right,
            bottom,
        })
    }

    fn coordinate(&self, field: &str) -> Result<usize, String> {
        let raw = self
            .0
            .field(field)
            .and_then(Value::integer)
            .ok_or_else(|| format!("window.drag region missing integer {field}"))?;
        usize::try_from(raw).map_err(|_| format!("window.drag {field} exceeds usize"))
    }

    fn progress(&self) -> Result<Progress, String> {
        let state = self
            .0
            .field("state")
            .and_then(Value::text)
            .ok_or_else(|| "window.progress requires a string state".to_string())?;
        match state {
            "none" => Ok(Progress::None),
            "indeterminate" => Ok(Progress::Indeterminate),
            "normal" => {
                let (current, total) = self.bounds()?;
                Ok(Progress::Normal { current, total })
            }
            "paused" => {
                let (current, total) = self.bounds()?;
                Ok(Progress::Paused { current, total })
            }
            "error" => {
                let (current, total) = self.bounds()?;
                Ok(Progress::Error { current, total })
            }
            _ => Err("unsupported window.progress state".into()),
        }
    }

    fn bounds(&self) -> Result<(u64, u64), String> {
        let current = self
            .0
            .field("current")
            .and_then(Value::integer)
            .ok_or_else(|| "window.progress requires integer current".to_string())?;
        let total = self
            .0
            .field("total")
            .and_then(Value::integer)
            .ok_or_else(|| "window.progress requires integer total".to_string())?;
        if total == 0 || current > total {
            return Err("window.progress requires total > 0 and current <= total".into());
        }
        Ok((current, total))
    }

    fn prompt(&self) -> Result<Prompt, String> {
        if matches!(self.0, Value::Null) {
            return Ok(Prompt::default());
        }
        let map = self
            .0
            .object()
            .ok_or_else(|| "dialog payload must be null or an object".to_string())?;
        let mut parsed = Prompt::default();
        for (key, value) in map {
            match key.as_str() {
                "title" => parsed.title = Fields(value).optional("title")?,
                "directory" => parsed.directory = Fields(value).optional("directory")?,
                "name" => parsed.name = Fields(value).optional("name")?,
                "multiple" => {
                    parsed.multiple = value
                        .boolean()
                        .ok_or_else(|| "dialog multiple must be boolean".to_string())?;
                }
                "filters" => parsed.filters = Fields(value).filters()?,
                _ => return Err(format!("unknown dialog field: {key}")),
            }
        }
        Ok(parsed)
    }

    fn optional(&self, field: &str) -> Result<Option<String>, String> {
        match self.0 {
            Value::Null => Ok(None),
            Value::Text(text) if text.len() <= TITLE && !text.chars().any(char::is_control) => {
                Ok(Some(text.clone()))
            }
            _ => Err(format!("invalid dialog {field}")),
        }
    }

    fn filters(&self) -> Result<Vec<Filter>, String> {
        let items = self
            .0
            .array()
            .ok_or_else(|| "dialog filters must be an array".to_string())?;
        if items.len() > FILTERS {
            return Err("dialog filters exceed budget".into());
        }
        let mut parsed = Vec::with_capacity(items.len());
        for item in items {
            let label = item
                .field("label")
                .and_then(Value::text)
                .filter(|text| {
                    !text.is_empty() && text.len() <= TITLE && !text.chars().any(char::is_control)
                })
                .ok_or_else(|| "dialog filter requires valid label".to_string())?;
            let raw = item
                .field("extensions")
                .and_then(Value::array)
                .ok_or_else(|| "dialog filter requires extensions array".to_string())?;
            if raw.len() > EXTENSIONS {
                return Err("dialog filter extensions exceed budget".into());
            }
            let mut extensions = Vec::with_capacity(raw.len());
            for entry in raw {
                let extension = entry
                    .text()
                    .filter(|text| {
                        !text.is_empty() && text.len() <= 32 && !text.chars().any(char::is_control)
                    })
                    .ok_or_else(|| "invalid dialog extension".to_string())?;
                extensions.push(extension.into());
            }
            parsed.push(Filter {
                label: label.into(),
                extensions,
            });
        }
        Ok(parsed)
    }
}

fn encode(selection: Selection) -> Value {
    let (cancelled, paths) = match selection {
        Selection::Cancelled => (true, Vec::new()),
        Selection::Paths(paths) => (false, paths.into_iter().map(Value::Text).collect()),
    };
    Value::Object(BTreeMap::from([
        ("cancelled".into(), Value::Bool(cancelled)),
        ("paths".into(), Value::Array(paths)),
    ]))
}
