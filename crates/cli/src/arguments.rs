use std::ffi::OsString;
use std::path::PathBuf;

use plectron::contract::Fault;
use plectron::ipc::Command;

pub enum Action {
    Dev {
        root: PathBuf,
    },
    Build {
        entry: PathBuf,
        output: PathBuf,
    },
    Observe {
        pid: u32,
        id: u64,
        output: PathBuf,
    },
    Export {
        pid: u32,
        unit: u64,
        output: PathBuf,
    },
    Run {
        path: PathBuf,
        persistent: bool,
    },
    List,
    Capture {
        backend: String,
        root: PathBuf,
        output: PathBuf,
        scale: f64,
        entry: String,
    },
    Control {
        pid: u32,
        command: Command,
    },
}

#[locus::trace(with = plumb::observation::view())]
pub fn parse(arguments: &[OsString]) -> Result<Action, Fault> {
    match arguments {
        [name, start, root] if name == "dev" && start == "start" => {
            Ok(Action::Dev { root: root.into() })
        }
        [name, entry, output] if name == "build" => Ok(Action::Build {
            entry: entry.into(),
            output: output.into(),
        }),
        [name, pid, id, output] if name == "observe" => Ok(Action::Observe {
            pid: number(pid)?
                .try_into()
                .map_err(|_| Fault("PID exceeds u32".into()))?,
            id: number(id)?,
            output: output.into(),
        }),
        [name, pid, unit, output] if name == "export" => Ok(Action::Export {
            pid: number(pid)?
                .try_into()
                .map_err(|_| Fault("PID exceeds u32".into()))?,
            unit: number(unit)?,
            output: output.into(),
        }),
        [name, backend, root, output] if name == "capture" => Ok(Action::Capture {
            backend: utf8(backend)?,
            root: root.into(),
            output: output.into(),
            scale: 1.0,
            entry: "index.html".into(),
        }),
        [name, backend, root, output, flag, scale] if name == "capture" && flag == "--scale" => {
            Ok(Action::Capture {
                backend: utf8(backend)?,
                root: root.into(),
                output: output.into(),
                entry: "index.html".into(),
                scale: utf8(scale)?
                    .parse()
                    .map_err(|_| Fault("invalid device scale".into()))?,
            })
        }
        [name, backend, root, output, flag, entry, density, scale]
            if name == "capture" && flag == "--entry" && density == "--scale" =>
        {
            Ok(Action::Capture {
                backend: utf8(backend)?,
                root: root.into(),
                output: output.into(),
                entry: utf8(entry)?,
                scale: utf8(scale)?
                    .parse()
                    .map_err(|_| Fault("invalid device scale".into()))?,
            })
        }
        [name, backend, root, output, flag, entry] if name == "capture" && flag == "--entry" => {
            Ok(Action::Capture {
                backend: utf8(backend)?,
                root: root.into(),
                output: output.into(),
                entry: utf8(entry)?,
                scale: 1.0,
            })
        }
        [name] if name == "ps" => Ok(Action::List),
        [name, pid, rest @ ..] if name == "ctl" => Ok(Action::Control {
            pid: number(pid)?
                .try_into()
                .map_err(|_| Fault("PID exceeds u32".into()))?,
            command: command(rest)?,
        }),
        [name, flag, path] if name == "run" && flag == "--wait" => Ok(Action::Run {
            path: path.into(),
            persistent: true,
        }),
        [name, path] if name == "run" => Ok(Action::Run {
            path: path.into(),
            persistent: false,
        }),
        [path] if path.to_string_lossy().starts_with('-') => Err(Fault(format!(
            "unknown option '{}'; {USAGE}",
            path.to_string_lossy()
        ))),
        [path] => Ok(Action::Run {
            path: path.into(),
            persistent: false,
        }),
        _ => Err(Fault(USAGE.into())),
    }
}

const USAGE: &str = "usage: plectron dev start <app-directory> | <script.js> | run [--wait] <script.js> | build <entry.js|entry.pt> <output.js> | capture <cpu|metal> <root> <output.ppm> [--entry <path>] [--scale <factor>] | export <pid> <unit> <output.json> | observe <pid> <session> <output.json> | ps | ctl <pid> <command>";

fn command(arguments: &[OsString]) -> Result<Command, Fault> {
    match arguments {
        [name, command] if name == "query" => Ok(Command::Query {
            command: serde_json::from_str(&utf8(command)?)
                .map_err(|error| Fault(error.to_string()))?,
        }),
        [name, command] if name == "observe" => Ok(Command::Observe {
            command: serde_json::from_str(&utf8(command)?)
                .map_err(|error| Fault(error.to_string()))?,
        }),
        [name, command] if name == "config" => Ok(Command::Config {
            command: serde_json::from_str(&utf8(command)?)
                .map_err(|error| Fault(error.to_string()))?,
        }),
        [name, unit, id, policy] if name == "cache" => Ok(Command::Cache {
            unit: number(unit)?,
            id: number(id)?,
            command: serde_json::from_str(&utf8(policy)?)
                .map_err(|error| Fault(error.to_string()))?,
        }),
        [name] => match name.to_str() {
            Some("control") => Ok(Command::Control {}),
            Some("hold") => Ok(Command::Hold { unit: None }),
            Some("unhold") => Ok(Command::Unhold { unit: None }),
            Some("status") => Ok(Command::Status {}),
            Some("windows") => Ok(Command::Windows {}),
            Some("services") => Ok(Command::Services {}),
            Some("pause") => Ok(Command::Pause {}),
            Some("resume") => Ok(Command::Resume {}),
            Some("step") => Ok(Command::Step {}),
            Some("shutdown") => Ok(Command::Shutdown {}),
            _ => Err(usage()),
        },
        [name, source] if name == "eval" => Ok(Command::Eval {
            source: source
                .to_str()
                .ok_or_else(|| Fault("source must be UTF-8".into()))?
                .into(),
        }),
        [name, unit] if name == "click" => Ok(Command::Click {
            unit: number(unit)?,
        }),
        [name, unit] if name == "hold" => Ok(Command::Hold {
            unit: Some(number(unit)?),
        }),
        [name, unit] if name == "unhold" => Ok(Command::Unhold {
            unit: Some(number(unit)?),
        }),
        [name, unit] if name == "page" => Ok(Command::Page {
            unit: number(unit)?,
        }),
        [name, unit, source] if name == "eval" => Ok(Command::Evaluate {
            unit: number(unit)?,
            source: utf8(source)?,
        }),
        [name, unit, delta] if name == "scroll" => Ok(Command::Scroll {
            unit: number(unit)?,
            delta: utf8(delta)?
                .parse()
                .map_err(|_| Fault("expected signed pixel delta".into()))?,
        }),
        [name, unit, x, y, delta] if name == "wheel" => Ok(Command::Wheel {
            unit: number(unit)?,
            x: coordinate(x)?,
            y: coordinate(y)?,
            delta: utf8(delta)?
                .parse()
                .map_err(|_| Fault("expected signed pixel delta".into()))?,
        }),
        [name, unit, scene, space, delta] if name == "pan" => Ok(Command::Pan {
            unit: number(unit)?,
            scene: number(scene)?,
            space: number(space)?,
            delta: utf8(delta)?
                .parse()
                .map_err(|_| Fault("expected signed pixel delta".into()))?,
        }),
        [name, unit, text] if name == "text" => Ok(Command::Input {
            unit: number(unit)?,
            input: webengine::keyboard::Input::Text(utf8(text)?),
        }),
        [name, unit, command] if name == "command" => Ok(Command::Edit {
            unit: number(unit)?,
            edit: webengine::keyboard::command(&utf8(command)?)?,
        }),
        [name, unit, key] if name == "key" => Ok(Command::Input {
            unit: number(unit)?,
            input: webengine::keyboard::Input::Key(webengine::keyboard::Stroke {
                chord: webengine::keyboard::Chord::parse(&utf8(key)?)
                    .map_err(|error| Fault(error.to_string()))?,
                repeat: false,
                text: String::new(),
            }),
        }),
        [name, unit, text] if name == "send" => Ok(Command::Send {
            unit: number(unit)?,
            text: utf8(text)?,
        }),
        [name, unit, x, y] if name == "click" => Ok(Command::Pointer {
            unit: number(unit)?,
            x: coordinate(x)?,
            y: coordinate(y)?,
        }),
        [name, unit] if name == "close" => Ok(Command::Close {
            unit: number(unit)?,
        }),
        _ => Err(usage()),
    }
}

fn number(value: &OsString) -> Result<u64, Fault> {
    value
        .to_str()
        .and_then(|text| text.parse().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| Fault("expected a positive numeric identifier".into()))
}

fn usage() -> Fault {
    Fault("commands: observe <json> | config <json> | cache <unit> <id> <json> | status | control | windows | services | page <unit> | pause | resume | hold [unit] | unhold [unit] | step | eval [unit] <source> | click <unit> [x y] | scroll <unit> <delta> | wheel <unit> <x> <y> <delta> | pan <unit> <scene> <space> <delta> | text <unit> <text> | key <unit> <chord> | command <unit> <command> | send <unit> <text> | close <unit> | shutdown".into())
}

fn utf8(value: &OsString) -> Result<String, Fault> {
    value
        .to_str()
        .map(String::from)
        .ok_or_else(|| Fault("expected UTF-8 text".into()))
}

fn coordinate(value: &OsString) -> Result<usize, Fault> {
    utf8(value)?
        .parse()
        .map_err(|_| Fault("expected nonnegative coordinate".into()))
}
