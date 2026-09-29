use super::error;
#[cfg(any(target_os = "macos", test))]
use ipcengine::file::Draft;
#[cfg(any(target_os = "macos", test))]
use ipcengine::stream::Transfer;
use ipcengine::{message::Connection, stream};
use plectron::{contract::Fault, ipc::Response};
use std::path::Path;
use std::time::Duration;

#[cfg(any(target_os = "macos", test))]
#[allow(dead_code)]
pub fn serve<T: serde::Serialize>(
    connection: &mut Connection,
    payload: (T, (&str, u32)),
    directory: &Path,
) -> Result<(), Fault> {
    let (bundle, (format, version)) = payload;
    let draft = Draft::create(directory, 268_435_456).map_err(error)?;
    let mut writer = std::io::BufWriter::with_capacity(stream::CHUNK, draft);
    serde_json::to_writer(&mut writer, &bundle).map_err(error)?;
    drop(bundle);
    let artifact = writer.into_inner().map_err(error)?.seal().map_err(error)?;
    let length = artifact.length();
    let transfer = Transfer::new(artifact.reader(), length);
    drop(artifact);
    connection.write(&Response { version: 1, result: Ok::<_, String>(serde_json::json!({"type":"transfer", "length":length, "format":format, "version":version, "transport":"binary"})) }, 65_536).map_err(error)?;
    transfer.serve(connection);
    Ok(())
}

pub fn export(directory: &Path, target: (u32, u64), output: &Path) -> Result<(), Fault> {
    receive(
        directory,
        (target.0, plectron::ipc::Command::Export { unit: target.1 }),
        output,
        ("plectron.snapshot", 2),
    )
}

pub fn receive(
    directory: &Path,
    target: (u32, plectron::ipc::Command),
    output: &Path,
    format: (&str, u32),
) -> Result<(), Fault> {
    use ipcengine::endpoint;
    use plectron::ipc::Request;
    let mut connection =
        Connection::connect(&endpoint::path(directory, target.0)).map_err(error)?;
    connection
        .write(&Request::new(target.1), 65_536)
        .map_err(error)?;
    let response: Response<serde_json::Value> = connection
        .read(65_536, Duration::from_secs(30))
        .map_err(error)?;
    if response.version != 1 {
        return Err(Fault("unsupported control response version".into()));
    }
    let header = response.result.map_err(Fault)?;
    if header["type"] != "transfer" || header["format"] != format.0 || header["version"] != format.1
    {
        return Err(Fault("unexpected export protocol".into()));
    }
    if header["transport"] != "binary" {
        return Err(Fault("unsupported export transport".into()));
    }
    let length = header["length"]
        .as_u64()
        .filter(|length| *length <= 268_435_456)
        .ok_or_else(|| Fault("invalid export length".into()))?;
    let monitor = ipcengine::observe::Monitor::default();
    let result = deliver(&mut connection, output, length, &monitor);
    eprintln!(
        "[export] {}",
        serde_json::to_string(&monitor.snapshot()).map_err(error)?
    );
    result
}
fn deliver(
    connection: &mut Connection,
    output: &Path,
    length: u64,
    monitor: &ipcengine::observe::Monitor,
) -> Result<(), Fault> {
    use std::io::Write;
    let mut delivery = ipcengine::file::Delivery::new(output, length).map_err(error)?;
    let mut client = stream::Client::new(connection, length, monitor);
    loop {
        let chunk = client.read(stream::CHUNK).map_err(error)?;
        delivery.write_all(&chunk.bytes).map_err(error)?;
        if chunk.eof {
            break;
        }
    }
    client.close().map_err(error)?;
    delivery.finish().map_err(error)
}
