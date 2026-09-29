mod device;
mod strategy;

use plectron::addons::control::Control;
use plectron::control::proxy::{Envelope, Proxy};
use plectron::platform::{Host, Platform};
use plectron::rendering::Solid;
use plectron::runtime::Runtime;
use serde_json::json;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use strategy::{STRATEGIES, Strategy};

const INTERVAL: Duration = Duration::from_millis(5);
const WARMUP: usize = 5;
const CASES: [(&str, &str, usize); 4] = [
    ("compute", "1;", 0),
    ("single", "console.log(1); 1;", 1),
    (
        "burst",
        "for (const value of [1, 1, 1, 1, 1, 1, 1, 1]) { console.log(value); } 1;",
        8,
    ),
    (
        "promise",
        "Promise.resolve(1).then(value => console.log(value)); 1;",
        1,
    ),
];

struct Worker {
    sender: Sender<observe::Envelope<String>>,
    receiver: Receiver<String>,
    requests: Receiver<Envelope>,
    join: std::thread::JoinHandle<()>,
}
impl Worker {
    fn spawn(strategy: Strategy) -> Self {
        let (proxy, requests) = Proxy::channel();
        let owner = std::thread::current();
        let proxy = if strategy.platform() {
            proxy.notified(shared::Notify::thread(owner.clone()))
        } else {
            proxy
        };
        let (sender, inbox) = mpsc::channel::<observe::Envelope<String>>();
        let (outbox, receiver) = mpsc::channel();
        let (ready, started) = mpsc::channel();
        let join = std::thread::spawn(move || {
            let mut runtime = Runtime::new(Control::new(proxy)).unwrap();
            runtime.execute(&fixtures::script("console", "")).unwrap();
            ready.send(()).unwrap();
            for task in inbox {
                let (source, mut scope) = task.receive("probe.execute");
                let result = runtime
                    .execute(&source)
                    .unwrap_or_else(|failure| {
                        panic!("{}: {}", source, runtime.diagnostic(&failure))
                    })
                    .text();
                runtime.checkpoint().unwrap();
                runtime.addons().host.finish().unwrap();
                scope.finish(true);
                drop(scope);
                outbox.send(result).unwrap();
                if strategy.completion() {
                    owner.unpark();
                }
            }
        });
        started.recv().unwrap();
        Self {
            sender,
            receiver,
            requests,
            join,
        }
    }
    fn stop(self) {
        drop(self.sender);
        self.join.join().unwrap();
    }
}
fn pause(notified: bool) {
    if notified {
        std::thread::park_timeout(INTERVAL);
    } else {
        std::thread::sleep(INTERVAL);
    }
}
fn execute(worker: &Worker, host: &mut impl Host, source: &str, notified: bool) -> (u128, usize) {
    let start = Instant::now();
    worker.sender.send(source.to_owned().into()).unwrap();
    let mut waits = 0;
    loop {
        for request in worker.requests.try_iter().take(16) {
            request.apply(host);
        }
        match worker.receiver.try_recv() {
            Ok(result) => {
                assert_eq!(result, "1");
                return (start.elapsed().as_nanos(), waits);
            }
            Err(mpsc::TryRecvError::Disconnected) => panic!("worker disconnected"),
            Err(mpsc::TryRecvError::Empty) => (),
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        pause(notified);
        waits += 1;
    }
}
fn run(
    case: (&str, &str, usize),
    strategy: Strategy,
    traced: bool,
    samples: usize,
) -> serde_json::Value {
    let logs = Rc::new(Cell::new(0));
    let mut host = Platform::new(device::Device(logs.clone()), Solid);
    let worker = Worker::spawn(strategy);
    let session = traced.then(|| {
        observe::Session::new(observe::Options {
            cpu: true,
            ..Default::default()
        })
    });
    let mut values = Vec::new();
    for index in 0..samples + WARMUP {
        let name = if index < WARMUP {
            "warmup"
        } else {
            "probe.action"
        };
        let mut scope = session.as_ref().map(|session| session.scope(name));
        let (wall, waits) = execute(&worker, &mut host, case.1, strategy.notified());
        if let Some(scope) = &mut scope {
            scope.finish(true);
        }
        if index >= WARMUP {
            values.push(json!({"wall":wall, "waits":waits}));
        }
    }
    assert_eq!(logs.get(), (samples + WARMUP) * case.2);
    worker.stop();
    let report = session.map(|session| {
        session.stop();
        let report = session.inspect();
        assert_eq!((report.open, report.dropped), (0, 0));
        assert!(
            report
                .records
                .iter()
                .all(|record| record.outcome == observe::Outcome::Success)
        );
        report
    });
    json!({"case":case.0, "mode":strategy, "traced":traced, "samples":values, "report":report})
}
fn idle(notified: bool) -> serde_json::Value {
    let session = observe::Session::new(observe::Options {
        cpu: true,
        ..Default::default()
    });
    let mut scope = session.scope("probe.idle");
    let start = Instant::now();
    let mut waits = 0;
    while start.elapsed() < Duration::from_millis(500) {
        pause(notified);
        waits += 1;
    }
    scope.finish(true);
    drop(scope);
    session.stop();
    json!({"notified":notified, "waits":waits, "report":session.inspect()})
}
#[test]
fn delivery() {
    for strategy in STRATEGIES {
        for case in CASES {
            run(case, strategy, true, 1);
        }
    }
}
#[test]
#[ignore]
fn experiment() {
    let output = std::path::PathBuf::from(
        std::env::var_os("PLECTRON_EXPERIMENT_OUTPUT").expect("output directory"),
    );
    std::fs::create_dir(&output).unwrap();
    let samples: usize = std::env::var("PLECTRON_EXPERIMENT_SAMPLES")
        .unwrap_or("40".into())
        .parse()
        .unwrap();
    assert!(samples > 0);
    let mut runs = Vec::new();
    for round in 0..3 {
        for mode in 0..8 {
            let variant = (round + mode) % 8;
            runs.extend(batch(round, variant, samples));
        }
    }
    let result = json!({
        "unit":"nanoseconds", "endpoint":"enqueue action through host-observed result after execute, checkpoint and finish",
        "interval":INTERVAL.as_nanos(), "warmup":WARMUP, "samples":samples, "rounds":3,
        "build":if cfg!(debug_assertions) { "debug" } else { "release" },
        "limits":["No native event loop, IPC client, window or render work", "In-process warm runtime; worker startup excluded", "Platform enqueue and result publication notifications independently varied", "Timeout fallback retained; idle wakeups not eliminated"],
        "runs":runs, "idle":[idle(false), idle(true)]
    });
    std::fs::write(
        output.join("experiment.json"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
}
fn batch(round: usize, variant: usize, samples: usize) -> Vec<serde_json::Value> {
    CASES
        .into_iter()
        .map(|case| {
            let result = run(case, STRATEGIES[variant % 4], variant >= 4, samples);
            json!({"round":round, "run":result})
        })
        .collect()
}
