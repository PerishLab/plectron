use ipcengine::{
    endpoint::{path, prepare},
    message::Connection,
};
use std::path::Path;
use std::time::Duration;

use plectron::contract::Fault;
use plectron::ipc::{Command, Request, Response};

use super::error;

pub fn send(
    directory: &Path,
    pid: u32,
    command: Command,
) -> Result<Response<serde_json::Value>, Fault> {
    prepare(directory).map_err(error)?;
    let mut stream = Connection::connect(&path(directory, pid)).map_err(error)?;
    stream
        .write(&Request::new(command), 65_536)
        .map_err(error)?;
    let response: Response<serde_json::Value> = stream
        .read(8_388_608, Duration::from_secs(6))
        .map_err(error)?;
    if response.version != 1 {
        return Err(Fault("unsupported control response version".into()));
    }
    Ok(response)
}

pub fn list(directory: &Path) -> Result<(), Fault> {
    prepare(directory).map_err(error)?;
    let mut pids = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(error)? {
        let name = entry.map_err(error)?.file_name();
        if let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_suffix(".sock"))
            .and_then(|pid| pid.parse::<u32>().ok())
        {
            pids.push(pid);
        }
    }
    pids.sort_unstable();
    for pid in pids {
        match send(directory, pid, Command::Status {}) {
            Ok(response) => println!("{}", serde_json::to_string(&response).map_err(error)?),
            Err(failure) => println!(
                "{}",
                serde_json::json!({"pid":pid,"error":failure.to_string()})
            ),
        }
    }
    Ok(())
}
