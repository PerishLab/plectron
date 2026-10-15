mod arguments;
mod build;
mod capture;
use plectron_cli::{ipc, project};

use arguments::Action;
use clap::Parser;
#[cfg(target_os = "linux")]
use platform::linux::Desktop;
#[cfg(target_os = "macos")]
use platform::macos::Desktop;
#[cfg(windows)]
use platform::windows::Desktop;
use plectron::contract::Fault;
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use plectron::ipc::{Response, Session};
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use plectron::platform::Platform;
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use plectron::rendering::Solid;
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "plectron", version = plumb::version!("PLECTRON"), after_help = plectron_locus::HELP)]
struct Cli {
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    arguments: Vec<std::ffi::OsString>,
}

fn main() -> ExitCode {
    if let Err(error) = plumb::identity!("PLECTRON") {
        eprintln!("plectron: {error}");
        return ExitCode::from(1);
    }
    let arguments = Cli::parse().arguments;
    let observer = plectron_locus::Run::start("model");
    let code = match start(arguments, observer.as_ref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("plectron: {error}");
            ExitCode::FAILURE
        }
    };
    if let Some(observer) = observer {
        observer.finish(if code == ExitCode::SUCCESS { 0 } else { 1 });
    }
    code
}

fn start(
    arguments: Vec<std::ffi::OsString>,
    observer: Option<&plectron_locus::Run>,
) -> Result<(), Fault> {
    let action = arguments::parse(&arguments, observer)?;
    if let Action::Build { entry, output } = &action {
        return build::run(entry, output, observer);
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
    let settings = plectron::settings::Settings::load()?;
    let directory = settings.control;
    let renderer = settings.renderer;
    match action {
        Action::Dev { root } => {
            let source = project::load(&root)?;
            run(source, &directory, (false, &renderer))
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
            let source = project::file(&path)?;
            run(source, &directory, (persistent, &renderer))
        }
    }
}

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
fn run(
    source: plectron::runtime::Script,
    directory: &Path,
    options: (bool, &str),
) -> Result<(), Fault> {
    let query = plectron::ipc::query::Store::default();
    let platform = Platform::observed(
        Desktop::new(options.1, |error| Fault(error.to_string()))?,
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

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn run(_: plectron::runtime::Script, _: &Path, _: (bool, &str)) -> Result<(), Fault> {
    Err(Fault(format!(
        "native desktop adapter is unavailable on {}",
        std::env::consts::OS
    )))
}
