use graphics::shared::{Backend, Cpu, Extent, Frame, Scene};
use ipcengine::{endpoint, message::Connection, stream};
use plectron::{
    ipc::{Command, Reply, Request, Response},
    resource::Services,
};
use plectron_cli::ipc;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use webengine::{Page, wire::Bundle};

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn page() -> Page {
    let root = fixtures::root();
    let mut services = Services::default();
    let service = services.serve(&root).unwrap();
    services.scheme(service, "app://export/").unwrap();
    let mut page = Page::new(
        "app://export/network/index.html",
        Extent {
            width: 720,
            height: 640,
        },
    )
    .unwrap();
    page.load(&mut services).unwrap();
    page
}
fn connect(directory: &Directory) -> Connection {
    Connection::connect(&endpoint::path(&directory.0, std::process::id())).unwrap()
}
fn request(connection: &mut Connection, command: Command) -> serde_json::Value {
    connection.write(&Request::new(command), 65_536).unwrap();
    let response: Response<serde_json::Value> =
        connection.read(8_388_608, Duration::from_secs(30)).unwrap();
    response.result.unwrap()
}
#[test]
fn export() {
    let directory =
        Directory(std::env::temp_dir().join(format!("plectron-export-{}", std::process::id())));
    let mut page = page();
    let frame = page.frame().unwrap();
    let pixels = Cpu.render(&frame).unwrap();
    let snapshot = page.inspect();
    let bundle = Bundle::new(snapshot.clone());
    let manifest = bundle.resources.manifest();
    assert!(manifest.fonts.values().sum::<usize>() > 8_388_608);
    assert!(!manifest.rasters.is_empty());
    assert!(manifest.fonts.len() < snapshot.fragments.len());
    assert!(serde_json::to_vec(&bundle.clone().inspect()).unwrap().len() < 65_536);
    let mut unresolved = bundle.clone();
    unresolved.resources = Default::default();
    assert!(unresolved.restore().is_err());
    page.close();
    let server = ipc::Server::bind(
        &directory.0,
        std::process::id(),
        (None, plectron::ipc::query::Store::default().reader()),
    )
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let worker = std::thread::spawn(move || handle(server, stopping, snapshot));
    let inspection = request(&mut connect(&directory), Command::Page { unit: 1 });
    assert_eq!(inspection["type"], "page");
    let mut stalled = connect(&directory);
    let header = request(&mut stalled, Command::Export { unit: 1 });
    assert!(header["length"].as_u64().unwrap() > 8_388_608);
    let started = Instant::now();
    assert_eq!(
        request(&mut connect(&directory), Command::Status {})["type"],
        "ack"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    let monitor = ipcengine::observe::Monitor::default();
    let mut client =
        stream::Client::new(&mut stalled, header["length"].as_u64().unwrap(), &monitor);
    assert!(!client.read(16).unwrap().eof);
    drop(client);
    drop(stalled);
    let output = directory.0.join("snapshot.json");
    ipc::export(&directory.0, (std::process::id(), 1), &output).unwrap();
    eprintln!(
        "snapshot export: {} bytes, {} fonts, {} rasters; inspection: {} bytes",
        std::fs::metadata(&output).unwrap().len(),
        manifest.fonts.len(),
        manifest.rasters.len(),
        serde_json::to_vec(&inspection).unwrap().len()
    );
    let saved: Bundle = serde_json::from_reader(std::io::BufReader::new(
        std::fs::File::open(&output).unwrap(),
    ))
    .unwrap();
    assert_eq!(saved.resources.manifest().fonts.len(), manifest.fonts.len());
    let restored = saved.restore().unwrap();
    let scene = Scene::new(
        restored.visual.extent,
        restored.painted.unwrap(),
        restored.items,
    )
    .unwrap();
    let replay = Frame::new(scene, restored.visual).unwrap();
    assert_eq!(Cpu.render(&replay).unwrap(), pixels);
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 2);
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    assert!(!endpoint::path(&directory.0, std::process::id()).exists());
}

fn handle(server: ipc::Server, stop: Arc<AtomicBool>, snapshot: webengine::Snapshot) {
    while !stop.load(Ordering::Relaxed) {
        if let Ok(envelope) = server.0.requests.recv_timeout(Duration::from_millis(10)) {
            let reply = match envelope.request.command {
                Command::Page { .. } | Command::Export { .. } => Reply::Page {
                    snapshot: Box::new(snapshot.clone()),
                },
                _ => Reply::Ack,
            };
            let _ = envelope.reply.send(Response {
                version: 1,
                result: Ok(reply),
            });
        }
    }
}

#[test]
fn passive() {
    let directory =
        Directory(std::env::temp_dir().join(format!("plectron-passive-{}", std::process::id())));
    let store = plectron::ipc::query::Store::default();
    let server =
        ipc::Server::bind(&directory.0, std::process::id(), (None, store.reader())).unwrap();
    let id = store
        .admit(plectron::ipc::query::Target::Task {
            unit: None,
            job: 50,
        })
        .unwrap();
    let response = request(
        &mut connect(&directory),
        Command::Query {
            command: plectron::ipc::query::Command::Read { record: id },
        },
    );
    assert_eq!(
        response["reply"]["status"]["record"]["state"]["type"],
        "pending"
    );
    assert!(server.0.requests.try_recv().is_err());
    store.finish(
        id,
        plectron::ipc::query::State::Complete {
            result: Ok("done".into()),
        },
        Some(Instant::now()),
    );
    let response = request(
        &mut connect(&directory),
        Command::Query {
            command: plectron::ipc::query::Command::Wait {
                record: id,
                timeout: 1000,
            },
        },
    );
    assert_eq!(
        response["reply"]["status"]["record"]["state"]["type"],
        "complete"
    );
    assert!(server.0.requests.try_recv().is_err());
    drop(server);
}
