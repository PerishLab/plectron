use super::{Connection, Envelope};
use crate::transport::{Listener, Socket};
use crate::{Fault, endpoint};
use std::net::Shutdown;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const IDLE: Duration = Duration::from_millis(5);

type Handler<Q, R> = Arc<dyn Fn(Connection, mpsc::SyncSender<Envelope<Q, R>>) + Send + Sync>;
pub struct Server<Q, R> {
    pub requests: mpsc::Receiver<Envelope<Q, R>>,
    path: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl<Q: Send + 'static, R: Send + 'static> Server<Q, R> {
    pub fn bind(
        directory: &Path,
        pid: u32,
        handler: impl Fn(Connection, mpsc::SyncSender<Envelope<Q, R>>) + Send + Sync + 'static,
    ) -> Result<Self, Fault> {
        endpoint::prepare(directory)?;
        let path = endpoint::path(directory, pid);
        let listener = Listener::bind(&path)?;
        let (sender, requests) = mpsc::sync_channel(16);
        let mut server = Self {
            requests,
            path,
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        };
        let stop = server.stop.clone();
        server.worker = Some(thread::Builder::new().name("ipc".into()).spawn(move || {
            serve(listener, sender, stop, Arc::new(handler));
        })?);
        Ok(server)
    }
}
impl<Q, R> Drop for Server<Q, R> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
struct Worker {
    socket: Socket,
    thread: JoinHandle<()>,
}
fn serve<Q: Send + 'static, R: Send + 'static>(
    listener: Listener,
    sender: mpsc::SyncSender<Envelope<Q, R>>,
    stop: Arc<AtomicBool>,
    handler: Handler<Q, R>,
) {
    let mut workers: Vec<Worker> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        reap(&mut workers);
        match listener.accept() {
            Ok(stream) if workers.len() < 16 => {
                workers.extend(spawn(stream, sender.clone(), handler.clone()));
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let _ = listener.wait(Instant::now() + IDLE);
            }
            Err(_) => break,
        }
    }
    drain(&mut workers);
    for worker in &workers {
        let _ = worker.socket.shutdown(Shutdown::Both);
    }
    for worker in workers {
        let _ = worker.thread.join();
    }
}
fn spawn<Q: Send + 'static, R: Send + 'static>(
    stream: Socket,
    sender: mpsc::SyncSender<Envelope<Q, R>>,
    handler: Handler<Q, R>,
) -> Result<Worker, Fault> {
    let socket = crate::transport::duplicate(&stream)?;
    let connection = Connection::new(stream)?;
    let thread = thread::Builder::new()
        .name("ipc-connection".into())
        .spawn(move || handler(connection, sender))?;
    Ok(Worker { socket, thread })
}
fn reap(workers: &mut Vec<Worker>) {
    let mut index = 0;
    while index < workers.len() {
        if workers[index].thread.is_finished() {
            let worker = workers.swap_remove(index);
            let _ = worker.thread.join();
        } else {
            index += 1;
        }
    }
}

fn drain(workers: &mut Vec<Worker>) {
    let deadline = std::time::Instant::now() + Duration::from_millis(100);
    while !workers.is_empty() && std::time::Instant::now() < deadline {
        reap(workers);
        thread::sleep(Duration::from_millis(2));
    }
}
