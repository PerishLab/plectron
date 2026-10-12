#[path = "worker/cache.rs"]
mod cache;
#[path = "worker/collection.rs"]
mod collection;
#[path = "worker/observation.rs"]
mod observation;
#[path = "worker/scrolling.rs"]
mod scrolling;
#[path = "worker/source.rs"]
mod source;
#[path = "worker/viewport.rs"]
mod viewport;
use graphics::shared::{Backend, Cpu, Extent};
use source::Source;
use std::time::{Duration, Instant};
use webengine::{
    Action, Fault, Phase, Snapshot, State, Worker,
    resource::{Provider, Response},
};
#[path = "worker/waiting.rs"]
mod waiting;
use waiting::{phase, ready, serial, until};
fn evaluate(worker: &mut Worker, source: &str) -> u64 {
    let id = worker.submit(Action::Evaluate(source.into())).unwrap();
    until(worker, |snapshot| phase(snapshot, id) == Phase::Complete);
    id
}
#[test]
fn blocked() {
    let _guard = serial();
    let mut worker = ready();
    evaluate(
        &mut worker,
        "let count = 0; const a = document.getElementById('a'); a.addEventListener('click', function (event) { count = count + 1; bridge.postMessage('a'); });",
    );
    let before = worker.inspect();
    let click = worker.hit(8, 8);
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    let clicked = worker.submit(Action::Dispatch(click)).unwrap();
    for offset in [16, 16, 16] {
        worker.scroll(offset).unwrap();
        let frame = worker.frame().unwrap();
        let image = Cpu.render(&frame).unwrap();
        worker.acknowledge(frame.id()).unwrap();
        assert_eq!(frame.scene().id(), before.scene.unwrap());
        assert_eq!(
            image.extent,
            Extent {
                width: 64,
                height: 32
            }
        );
        worker.poll().unwrap();
        assert_eq!(phase(&worker.inspect(), hold), Phase::Held);
        assert_eq!(phase(&worker.inspect(), clicked), Phase::Queued);
    }
    assert_eq!(worker.inspect().shown.unwrap().offset(0), 48);
    worker.unhold().unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, clicked) == Phase::Complete
    });
    assert_eq!(worker.messages(), ["a"]);
    let id = evaluate(&mut worker, "count;");
    let snapshot = worker.inspect();
    assert_eq!(
        snapshot
            .execution
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .result()
            .cloned(),
        Some(Ok("1".into()))
    );
    assert_eq!(snapshot.visual.offset(0), 48);
    scrolling::verify(&mut worker);
    observation::ownership(&mut worker);
    cache::ordered(&mut worker);
    worker.close();
}
#[test]
fn closing() {
    let _guard = serial();
    let mut worker = ready();
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    worker.scroll(32).unwrap();
    let frame = worker.frame().unwrap();
    let queued = worker
        .submit(Action::Evaluate("bridge.postMessage('late');".into()))
        .unwrap();
    let start = Instant::now();
    worker.close();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(worker.state(), State::Closed);
    assert!(worker.hold().is_err());
    assert!(worker.acknowledge(frame.id()).is_err());
    worker.poll().unwrap();
    assert!(worker.messages().is_empty());
    assert_eq!(phase(&worker.inspect(), queued), Phase::Cancelled);
    assert_eq!(Cpu.render(&frame).unwrap().pixels[20 * 64 + 60], 0x445566);
}
#[test]
fn pressure() {
    let _guard = serial();
    let mut first = ready();
    let mut second = ready();
    let hold = first.hold().unwrap();
    until(&mut first, |snapshot| phase(snapshot, hold) == Phase::Held);
    for _ in 0..8 {
        first.submit(Action::Evaluate("1;".into())).unwrap();
    }
    assert!(first.submit(Action::Evaluate("2;".into())).is_err());
    let id = evaluate(&mut second, "42;");
    assert_eq!(phase(&second.inspect(), id), Phase::Complete);
    assert_eq!(phase(&first.inspect(), hold), Phase::Held);
    first.close();
    second.close();
}
#[test]
fn partial() {
    let _guard = serial();
    let mut worker = ready();
    let before = worker.inspect();
    let id = evaluate(
        &mut worker,
        "document.getElementById('a').setText('changed'); missing();",
    );
    let failed = worker.inspect();
    assert!(failed.revision > before.revision);
    assert_eq!(failed.scene, before.scene);
    assert_eq!(failed.update.committed, before.revision);
    assert!(failed.nodes.iter().any(|node| node.content == "changed"));
    assert!(
        failed
            .execution
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .result()
            .unwrap()
            .is_err()
    );
    assert_eq!(worker.errors().len(), 1);
    evaluate(
        &mut worker,
        "document.getElementById('body').removeChild(document.getElementById('c'));",
    );
    assert_ne!(worker.inspect().scene, before.scene);
    worker.scroll(64).unwrap();
    assert_eq!(worker.inspect().visual.offset(0), 32);
    collection::visible(&mut worker);
    collection::pressure(&mut worker);
    worker.close();
}

struct Tracked {
    owner: std::thread::ThreadId,
    dropped: std::sync::mpsc::Sender<()>,
}
impl Provider for Tracked {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if matches!(request.kind, webengine::resource::Kind::Font) {
            let name = request.url.path().rsplit('/').next().unwrap();
            return Ok(Response {
                status: 200,
                mime: "font/ttf".into(),
                body: ::fixtures::bytes(name).to_vec(),
            });
        }
        let response: Result<Response, Fault> = {
            let url = &request.url;
            assert_ne!(self.owner, std::thread::current().id());
            Source.get(url)
        };
        let mut response = response?;
        if response.mime == "text/css" {
            response.body = ::fixtures::stylesheet(&response.body);
        }
        Ok(response)
    }
}
impl Drop for Tracked {
    fn drop(&mut self) {
        let _ = self.dropped.send(());
    }
}
#[test]
fn retirement() {
    let _guard = serial();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut worker = Worker::spawn(
        "app://worker/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        Tracked {
            owner: std::thread::current().id(),
            dropped: sender,
        },
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    worker.close();
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
}

#[test]
fn convergence() {
    let _guard = serial();
    let mut worker = ready();
    let held = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, held) == Phase::Held);
    worker.scroll(64).unwrap();
    let inflight = worker.frame().unwrap();
    let id = worker
        .submit(Action::Evaluate(
            "document.getElementById('body').removeChild(document.getElementById('c'));".into(),
        ))
        .unwrap();
    worker.unhold().unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, id) == Phase::Complete
    });
    assert_eq!(worker.inspect().visual.offset(0), 32);
    assert!(worker.frame().is_none());
    worker.acknowledge(inflight.id()).unwrap();
    assert_eq!(worker.inspect().shown.unwrap().offset(0), 64);
    let latest = worker.frame().unwrap();
    assert_ne!(latest.scene().id(), inflight.scene().id());
    assert_eq!(latest.visual().offset(0), 32);
    worker.acknowledge(latest.id()).unwrap();
    worker.close();
    assert_eq!(Cpu.render(&latest).unwrap().pixels[20 * 64 + 60], 0x445566);
}

#[test]
fn editing() {
    let _guard = serial();
    let mut worker = ready();
    evaluate(
        &mut worker,
        "const editor = document.createElement('input'); document.getElementById('body').appendChild(editor); editor.focus(); editor.addEventListener('input', () => { bridge.postMessage(editor.getValue()); });",
    );
    let held = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, held) == Phase::Held);
    let first = worker
        .submit(Action::Edit(webengine::Edit::Insert("queued".into())))
        .unwrap();
    let last = worker
        .submit(Action::Edit(webengine::Edit::Backward))
        .unwrap();
    assert_eq!(phase(&worker.inspect(), first), Phase::Queued);
    assert!(worker.messages().is_empty());
    worker.scroll(16).unwrap();
    worker.unhold().unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, first) == Phase::Complete
    });
    assert_eq!(worker.messages(), ["queued"]);
    until(&mut worker, |snapshot| {
        phase(snapshot, last) == Phase::Complete
    });
    assert_eq!(worker.messages(), ["queue"]);
    let snapshot = worker.inspect();
    assert_eq!(snapshot.visual.offset(0), 16);
    let editor = snapshot
        .nodes
        .iter()
        .find(|node| node.id == snapshot.focused.unwrap())
        .unwrap();
    assert_eq!(editor.editor.as_ref().unwrap().text, "queue");
    assert_eq!(editor.editor.as_ref().unwrap().selection.focus, 5);
    assert_eq!(snapshot.painted, snapshot.revision);
    worker.close();
    assert!(
        worker
            .submit(Action::Edit(webengine::Edit::Insert("late".into())))
            .is_err()
    );
}
