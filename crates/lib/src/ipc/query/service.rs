use super::{Command, Reader, Reply, State, Status};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CAPACITY: usize = 32;
const TIMEOUT: u64 = 2000;
struct Request {
    command: Command,
    deadline: Instant,
    reply: SyncSender<Result<Reply, String>>,
}
pub struct Service {
    sender: SyncSender<Request>,
    thread: Option<JoinHandle<()>>,
    closed: Arc<AtomicBool>,
}
impl Service {
    pub fn new(store: Reader) -> Result<Self, String> {
        let (sender, inbox) = mpsc::sync_channel(CAPACITY);
        let closed = Arc::new(AtomicBool::new(false));
        let stop = closed.clone();
        let (ready, started) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("plectron-query".into())
            .spawn(move || {
                let result = store.attach(thread::current());
                let valid = result.is_ok();
                let _ = ready.send(result);
                if valid {
                    run(store, inbox, stop);
                }
            })
            .map_err(|error| error.to_string())?;
        started.recv().map_err(|error| error.to_string())??;
        Ok(Self {
            sender,
            thread: Some(thread),
            closed,
        })
    }
    pub fn ask(&self, command: Command) -> Result<Reply, String> {
        let timeout = match command {
            Command::Wait { timeout, .. } if timeout > TIMEOUT => {
                return Err(format!("query wait exceeds {TIMEOUT} ms"));
            }
            Command::Wait { timeout, .. } => timeout,
            _ => 0,
        };
        let (reply, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Request {
                command,
                deadline: Instant::now() + Duration::from_millis(timeout),
                reply,
            })
            .map_err(|_| "query mailbox unavailable")?;
        self.thread
            .as_ref()
            .ok_or("query service closed")?
            .thread()
            .unpark();
        receiver
            .recv_timeout(Duration::from_millis(TIMEOUT + 1000))
            .map_err(|error| error.to_string())?
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}
fn run(store: Reader, inbox: Receiver<Request>, closed: Arc<AtomicBool>) {
    let mut pending = Vec::new();
    loop {
        let stopping = closed.load(Ordering::Acquire);

        pending.retain(|request| !respond(&store, request, stopping));
        for request in inbox.try_iter().take(CAPACITY) {
            if respond(&store, &request, stopping) {
                continue;
            }
            if pending.len() < CAPACITY {
                pending.push(request);
            } else {
                let _ = request
                    .reply
                    .try_send(Err("query wait capacity exhausted".into()));
            }
        }
        if stopping {
            store.detach();
            break;
        }
        let deadline = pending.iter().map(|request| request.deadline).min();
        match deadline {
            Some(deadline) => {
                thread::park_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => thread::park(),
        }
    }
}
fn respond(store: &Reader, request: &Request, stopping: bool) -> bool {
    let mut reply = store.read(&request.command);
    reply.closed |= stopping;
    let pending =
        matches!(&reply.status, Status::Record { record } if record.state == State::Pending);
    let waiting = matches!(request.command, Command::Wait { .. }) && pending && !reply.closed;
    if waiting && Instant::now() < request.deadline {
        return false;
    }
    reply.timedout = waiting;
    let _ = request.reply.try_send(Ok(reply));
    true
}
