use super::{error, transfer};
use ipcengine::message::{self, Connection, Envelope};
use plectron::contract::Fault;
use plectron::ipc::{Command, Reply, Request, Response};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub struct Server(pub message::Server<Request, Response>);
impl Server {
    pub fn bind(
        directory: &Path,
        pid: u32,
        options: (Option<shared::Notify>, plectron::ipc::query::Reader),
    ) -> Result<Self, Fault> {
        let storage = directory.to_path_buf();
        let queries = plectron::ipc::query::Service::new(options.1).map_err(error)?;
        let server = message::Server::bind(directory, pid, move |connection, sender| {
            serve(connection, sender, &storage, (options.0.as_ref(), &queries))
        })
        .map_err(error)?;
        eprintln!(
            "[control] pid={pid} socket={}",
            ipcengine::endpoint::path(directory, pid).display()
        );
        Ok(Self(server))
    }
}
fn serve(
    mut connection: Connection,
    sender: mpsc::SyncSender<Envelope<Request, Response>>,
    directory: &Path,
    services: (Option<&shared::Notify>, &plectron::ipc::query::Service),
) {
    if let Err(error) = exchange(&mut connection, &sender, directory, services) {
        let _ = connection.write(&Response::<()>::error(error), 65_536);
    }
}
fn exchange(
    connection: &mut Connection,
    sender: &mpsc::SyncSender<Envelope<Request, Response>>,
    directory: &Path,
    services: (Option<&shared::Notify>, &plectron::ipc::query::Service),
) -> Result<(), Fault> {
    let request: Request = connection
        .read(65_536, Duration::from_secs(2))
        .map_err(error)?;
    if request.version != 1 {
        return Err(Fault("unsupported protocol version".into()));
    }
    if let Command::Query { command } = request.command {
        let result = services.1.ask(command).map(|reply| Reply::Query { reply });
        return connection
            .write(&Response { version: 1, result }, 65_536)
            .map_err(error);
    }
    let export = matches!(request.command, Command::Export { .. });
    let response = message::ask(sender, request, services.0).map_err(error)?;
    if export && let Ok(Reply::Page { snapshot }) = response.result {
        return transfer::serve(
            connection,
            (
                webengine::wire::Bundle::new(*snapshot),
                ("plectron.snapshot", 2),
            ),
            directory,
        );
    }
    if let Ok(Reply::Observation { report }) = response.result {
        return transfer::serve(connection, (report, ("plectron.observation", 1)), directory);
    }
    let result = response.result.map(|reply| match reply {
        Reply::Page { snapshot } => {
            let inspection = webengine::wire::Bundle::new(*snapshot).inspect();
            serde_json::json!({ "type": "page", "snapshot": inspection.snapshot, "resources": inspection.resources })
        }
        reply => serde_json::to_value(reply).expect("protocol values serialize"),
    });
    connection
        .write(
            &Response {
                version: response.version,
                result,
            },
            8_388_608,
        )
        .map_err(error)
}
