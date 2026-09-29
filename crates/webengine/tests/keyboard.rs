use graphics::shared::Extent;
use std::time::{Duration, Instant};
use webengine::{
    Action, Fault, Phase, Snapshot, State, Worker,
    keyboard::{Chord, Input, Stroke},
    resource::{Provider, Response},
};

struct Source;
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script("document, shortcuts, bridge", "").into_bytes(),
            });
        }
        Ok(Response {
            status: 200,
            mime: "text/html".into(),
            body: b"<head><script src='./main.js'></script></head><body id='body'><input id='title'></body>".to_vec(),
        })
    }
}
fn until(worker: &mut Worker, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.poll().unwrap();
        let snapshot = worker.inspect();
        if predicate(&snapshot) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker timeout: {:?}",
            snapshot.execution
        );
        assert!(worker.errors().is_empty());
        worker.messages();
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn phase(snapshot: &Snapshot, id: u64) -> Phase {
    snapshot
        .execution
        .as_ref()
        .unwrap()
        .jobs
        .iter()
        .find(|job| job.id == id)
        .unwrap()
        .phase()
}
fn complete(snapshot: &Snapshot, id: u64) -> bool {
    let job = snapshot
        .execution
        .as_ref()
        .unwrap()
        .jobs
        .iter()
        .find(|job| job.id == id);
    job.is_some_and(|job| match job.result() {
        Some(Ok(_)) => true,
        Some(Err(fault)) => panic!("evaluation failed: {fault}"),
        None => false,
    })
}
fn key() -> Action {
    Action::Input(Input::Key(Stroke {
        chord: Chord::parse("Primary+KeyK").unwrap(),
        repeat: false,
        text: "k".into(),
    }))
}
#[test]
fn ownership() {
    let mut worker = Worker::spawn(
        "app://worker/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        Source,
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let setup = worker.submit(Action::Evaluate("const title = document.getElementById('title'); title.focus(); title.setValue('draft'); const group = shortcuts.register('editor', 0, [{chord: 'Primary+KeyK', command: 'selectAll'}]);".into())).unwrap();
    until(&mut worker, |snapshot| complete(snapshot, setup));
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    let select = worker.submit(key()).unwrap();
    let remove = worker
        .submit(Action::Evaluate("group.dispose();".into()))
        .unwrap();
    let fallback = worker.submit(key()).unwrap();
    assert_eq!(phase(&worker.inspect(), select), Phase::Queued);
    worker.unhold().unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, fallback) == Phase::Complete
    });
    let snapshot = worker.inspect();
    assert_eq!(phase(&snapshot, remove), Phase::Complete);
    let jobs = &snapshot.execution.as_ref().unwrap().jobs;
    assert_eq!(
        jobs.iter()
            .find(|job| job.id == select)
            .unwrap()
            .result()
            .unwrap()
            .as_ref()
            .unwrap(),
        "handled=true consumed=true"
    );
    assert_eq!(
        jobs.iter()
            .find(|job| job.id == fallback)
            .unwrap()
            .result()
            .unwrap()
            .as_ref()
            .unwrap(),
        "handled=false consumed=false"
    );
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .find_map(|node| node.editor.as_ref())
            .unwrap()
            .text,
        "k"
    );
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    let queued = worker.submit(key()).unwrap();
    worker.close();
    assert_eq!(phase(&worker.inspect(), queued), Phase::Cancelled);
    assert!(worker.submit(key()).is_err());
}

#[test]
fn diagnostics() {
    let mut worker = Worker::spawn(
        "app://worker/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        Source,
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let setup = worker.submit(Action::Evaluate("const title = document.getElementById('title'); title.focus(); const subscription = title.addEventListener('beforeinput', () => { throw Error('reported'); });".into())).unwrap();
    until(&mut worker, |snapshot| complete(snapshot, setup));
    let ready = worker.inspect();
    assert_eq!(ready.reuse.computed, 0);
    assert_eq!(ready.reuse.retained, 1);
    assert_eq!(ready.reuse.hits, 0);
    assert_eq!(Some(ready.reuse.revision), ready.revision);
    let edit = worker
        .submit(Action::Edit(webengine::Edit::Insert("retained".into())))
        .unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, edit) == Phase::Complete
    });
    let snapshot = worker.inspect();
    let record = snapshot
        .events
        .iter()
        .find(|record| record.kind == "beforeinput")
        .unwrap();
    assert_eq!(record.job, Some(edit));
    assert_eq!(record.errors, 1);
    assert_eq!(record.action, webengine::Effect::Applied);
    assert!(worker.errors().is_empty());
    assert_eq!(snapshot.revision, snapshot.painted);
    let replacement = worker.submit(Action::Evaluate("subscription.dispose(); title.addEventListener('beforeinput', () => { bridge.postMessage(1); });".into())).unwrap();
    until(&mut worker, |snapshot| complete(snapshot, replacement));
    let edit = worker
        .submit(Action::Edit(webengine::Edit::Insert("lost".into())))
        .unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, edit) == Phase::Complete
    });
    assert!(!worker.errors().is_empty());
    let record = worker.inspect().events.last().unwrap().clone();
    assert_eq!(record.job, Some(edit));
    assert_eq!(record.action, webengine::Effect::Failed);
    worker.close();
    assert!(worker.inspect().events.last().unwrap().failure.is_some());
}
