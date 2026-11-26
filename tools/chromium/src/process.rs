use plectron_chromium::Result;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

pub const TIMEOUT: Duration = Duration::from_secs(120);

pub fn text(program: &str, args: &[&str], directory: &std::path::Path) -> Result<String> {
    let output = capture(
        Command::new(program).args(args).current_dir(directory),
        TIMEOUT,
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

pub fn capture(command: &mut Command, timeout: Duration) -> Result<std::process::Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().ok_or("command has no stdout")?;
    let stderr = child.stderr.take().ok_or("command has no stderr")?;
    let out = thread::spawn(move || drain(stdout));
    let err = thread::spawn(move || drain(stderr));
    let status = match child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("command exceeded {} seconds", timeout.as_secs()));
        }
    };
    Ok(std::process::Output {
        status,
        stdout: out.join().map_err(|_| "stdout reader panicked")??,
        stderr: err.join().map_err(|_| "stderr reader panicked")??,
    })
}

fn drain(mut input: impl Read) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    input
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    Ok(data)
}

pub fn terminate(child: &mut Child) {
    #[cfg(unix)]
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGTERM,
    );
    #[cfg(not(unix))]
    let _ = child.kill();
    if child
        .wait_timeout(Duration::from_secs(5))
        .ok()
        .flatten()
        .is_none()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn readiness(server: &mut Child) -> Result<Value> {
    let stdout = server
        .stdout
        .take()
        .ok_or("static server has no readiness channel")?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    let line = receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "static server did not announce readiness".to_owned())?
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&line).map_err(|error| format!("server readiness: {error}"))
}

pub fn playwright(directory: &Path) -> Result<Command> {
    lookup(&plumb::config::search(directory)?)
}

fn lookup(search: &plumb::config::Search) -> Result<Command> {
    search.command("playwright-cli")
}

pub fn system(root: &Path) -> String {
    #[cfg(unix)]
    {
        if let Ok(value) = text("uname", &["-a"], root) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_owned();
            }
        }
    }
    #[cfg(not(unix))]
    let _ = root;
    plumb::config::platform()
}

pub fn server(root: &Path) -> Result<PathBuf> {
    let output = capture(
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "-p",
                "plectron-serve",
                "--message-format=json",
            ])
            .current_dir(root),
        TIMEOUT,
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&[output.stdout, output.stderr].concat()).into_owned());
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(artifact) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if artifact["reason"] == "compiler-artifact"
            && artifact["target"]["name"] == "plectron-serve"
            && let Some(executable) = artifact["executable"].as_str()
        {
            return Ok(executable.into());
        }
    }
    Err("cargo did not produce the plectron-serve executable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host() {
        let value = system(Path::new("."));
        assert!(!value.is_empty());
        #[cfg(windows)]
        assert_eq!(value, plumb::config::platform());
    }

    #[cfg(windows)]
    #[test]
    fn shim() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("playwright-cli.cmd");
        std::fs::write(&file, "@echo 0.1.17\r\n").unwrap();
        let search = plumb::config::Search::new(
            root.path(),
            Some(root.path().into()),
            Some(".CMD;.EXE".into()),
        )
        .unwrap();
        let mut command = lookup(&search).unwrap();
        let output = capture(command.arg("--version"), TIMEOUT).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "0.1.17");
    }
}
