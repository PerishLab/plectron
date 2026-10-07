mod arguments;
mod build;
mod capture;
use plectron_cli::{ipc, project};

use arguments::Action;
use clap::Parser;
use plectron::contract::Fault;
#[cfg(target_os = "macos")]
use plectron::ipc::{Response, Session};
#[cfg(target_os = "macos")]
use plectron::platform::Platform;
#[cfg(target_os = "macos")]
use plectron::rendering::Solid;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
#[cfg(target_os = "macos")]
const RENDERER: &str = "metal";
#[cfg(not(target_os = "macos"))]
const RENDERER: &str = "cpu";

#[derive(Parser)]
#[command(name = "plectron", version = plumb::version!("PLECTRON"))]
struct Cli {
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    arguments: Vec<std::ffi::OsString>,
}

fn main() -> ExitCode {
    match start(Cli::parse().arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("plectron: {error}");
            ExitCode::FAILURE
        }
    }
}

fn start(arguments: Vec<std::ffi::OsString>) -> Result<(), Fault> {
    let action = arguments::parse(&arguments)?;
    if let Action::Build { entry, output } = &action {
        return build::run(entry, output);
    }
    if let Action::Capture {
        backend,
        root,
        output,
        scale,
        entry,
    } = &action
    {
        return capture::run(backend, (root, entry), output, *scale);
    }
    let directory = std::env::var_os("PLECTRON_CONTROL_DIR")
        .map(PathBuf::from)
        .or_else(|| plumb::config::home().map(|home| home.join(".local/state/plectron")))
        .ok_or_else(|| Fault("HOME or PLECTRON_CONTROL_DIR is required".into()))?;
    match action {
        Action::Dev { root } => {
            let source = project::load(&root)?;
            let renderer = std::env::var("PLECTRON_RENDERER").unwrap_or_else(|_| RENDERER.into());
            run(&source, &directory, (false, &renderer))
        }
        Action::Build { .. } => unreachable!(),
        Action::Observe { pid, id, output } => ipc::observation(&directory, (pid, id), &output),
        Action::Export { pid, unit, output } => ipc::export(&directory, (pid, unit), &output),
        Action::Capture {
            backend,
            root,
            output,
            scale,
            entry,
        } => capture::run(&backend, (&root, &entry), &output, scale),
        Action::List => ipc::list(&directory),
        Action::Control { pid, command } => {
            let response = ipc::send(&directory, pid, command)?;
            println!("{}", serde_json::to_string(&response).map_err(ipc::error)?);
            response.result.map(|_| ()).map_err(Fault)
        }
        Action::Run { path, persistent } => {
            let source = std::fs::read_to_string(&path).map_err(ipc::error)?;
            let renderer = std::env::var("PLECTRON_RENDERER").unwrap_or_else(|_| RENDERER.into());
            run(&source, &directory, (persistent, &renderer))
        }
    }
}

#[cfg(target_os = "macos")]
fn run(source: &str, directory: &Path, options: (bool, &str)) -> Result<(), Fault> {
    let query = plectron::ipc::query::Store::default();
    let platform = Platform::observed(
        platform::macos::Desktop::new(options.1, |error| Fault(error.to_string()))?,
        Solid,
        query,
    )
    .workers();
    let notify = platform.notifier();
    let server = ipc::Server::bind(
        directory,
        std::process::id(),
        (Some(notify.clone()), platform.query().reader()),
    )?;
    let mut session = Session::spawn(
        platform,
        source,
        (std::process::id(), options.0),
        Some(notify),
    )?;
    while session.active() {
        let mut requests = 0;
        for envelope in server.0.requests.try_iter().take(16) {
            requests += 1;
            let response = if envelope.expired() {
                Response::error("request expired before execution")
            } else {
                session.handle(envelope.request)
            };
            let _ = envelope.reply.send(response);
        }
        session.tick()?;
        if requests < 16 && session.active() {
            session.wait()?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run(_: &str, _: &Path, _: (bool, &str)) -> Result<(), Fault> {
    Err(Fault(format!(
        "native desktop adapter is unavailable on {}",
        std::env::consts::OS
    )))
}
