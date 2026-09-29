use domengine::Extent;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use webengine::resource::{Provider, Response};
use webengine::{Fault, Worker};

struct Blocked {
    started: Sender<()>,
    release: Receiver<()>,
}
impl Provider for Blocked {
    fn request(&mut self, _: &webengine::resource::Request) -> Result<Response, Fault> {
        let _ = self.started.send(());
        let _ = self.release.recv();
        Err(Fault("provider stopped".into()))
    }
}
fn spawn() -> Result<(Worker, Receiver<()>, Sender<()>), Fault> {
    let (started, active) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let worker = Worker::spawn(
        "app://budget/index.html",
        Extent {
            width: 8,
            height: 8,
        },
        Blocked {
            started,
            release: wait,
        },
    )?;
    Ok((worker, active, release))
}
#[test]
fn retirement() {
    let mut workers = Vec::new();
    for _ in 0..8 {
        let (worker, active, release) = spawn().unwrap();
        active.recv_timeout(Duration::from_secs(5)).unwrap();
        workers.push((worker, release));
    }
    assert!(spawn().is_err());
    for (worker, _) in &mut workers {
        worker.close();
    }
    assert!(spawn().is_err());
    workers[0].1.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut replacement, active, release) = loop {
        if let Ok(worker) = spawn() {
            break worker;
        }
        assert!(Instant::now() < deadline, "worker permit was not returned");
        std::thread::sleep(Duration::from_millis(1));
    };
    active.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(spawn().is_err());
    replacement.close();
    release.send(()).unwrap();
    for (_, release) in workers.into_iter().skip(1) {
        release.send(()).unwrap();
    }
}
