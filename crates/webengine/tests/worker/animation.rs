use graphics::shared::Extent;
use std::time::{Duration, Instant};
use webengine::{
    Action, Fault, Phase, Snapshot, State, Worker,
    resource::{Provider, Response},
};

struct Source;
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script("document", "").into_bytes(),
            });
        }
        if request.url.path().ends_with("style.css") {
            return Ok(Response {
                status: 200,
                mime: "text/css".into(),
                body: b"@keyframes blink {from {opacity: 1;} 50% {opacity: 0;} to {opacity: 1;}} .pulse {width: 16px; height: 16px; background-color: #ff0000; animation-name: blink; animation-duration: 120ms; animation-iteration-count: infinite; animation-timing-function: step-end;}".to_vec(),
            });
        }
        Ok(Response {
            status: 200,
            mime: "text/html".into(),
            body: b"<html><head><link rel='stylesheet' href='./style.css'><script src='./main.js'></script></head><body id='body'><div id='pulse' class='pulse'></div></body></html>".to_vec(),
        })
    }
}

fn until(worker: &mut Worker, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.pulse(graphics::shared::Pulse::now()).unwrap();
        worker.poll().unwrap();
        if predicate(&worker.inspect()) {
            return;
        }
        assert!(Instant::now() < deadline, "worker deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn evaluate(worker: &mut Worker, source: &str) {
    let id = worker.submit(Action::Evaluate(source.into())).unwrap();
    until(worker, |snapshot| {
        snapshot
            .execution
            .as_ref()
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == id && job.phase() == Phase::Complete)
            .is_some_and(|job| match job.result().unwrap() {
                Ok(_) => true,
                Err(fault) => panic!("evaluation failed: {fault}"),
            })
    });
}

#[test]
fn lifecycle() {
    let mut worker = Worker::spawn(
        "app://animation/index.html",
        Extent {
            width: 32,
            height: 32,
        },
        Source,
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let initial = worker.inspect();
    until(&mut worker, |snapshot| {
        snapshot.computation.animations >= initial.computation.animations + 3
    });
    let animated = worker.inspect();
    let computation = animated.computation.since(initial.computation);
    assert_eq!(animated.revision, initial.revision);
    assert_ne!(animated.scene, initial.scene);
    assert_eq!(computation.layouts, 0);
    assert_eq!(computation.paints, 0);
    assert_eq!(computation.capture, 0);
    assert_eq!(computation.effects, computation.animations);
    let compositing = animated.compositing;
    assert!(compositing.updates <= computation.animations);
    assert!(compositing.effects <= computation.effects);
    assert_eq!(compositing.stale, 0);
    evaluate(
        &mut worker,
        "const pulse = document.getElementById('pulse'); pulse.style.setProperty('animation-play-state', 'paused');",
    );
    let paused = worker.inspect();
    std::thread::sleep(Duration::from_millis(180));
    worker.pulse(graphics::shared::Pulse::now()).unwrap();
    worker.poll().unwrap();
    assert_eq!(worker.inspect().computation, paused.computation);
    evaluate(
        &mut worker,
        "pulse.style.setProperty('animation-play-state', 'running');",
    );
    let resumed = worker.inspect();
    until(&mut worker, |snapshot| {
        let continued = snapshot.computation.since(resumed.computation);
        continued.animations >= 2
            && snapshot.compositing.updates + continued.coalesced
                >= resumed.compositing.updates + continued.animations
    });
    let continued = worker.inspect().computation.since(resumed.computation);
    assert_eq!(continued.layouts, 0);
    assert_eq!(continued.paints, 0);
    assert_eq!(continued.effects, continued.animations);
    let composed = worker.inspect().compositing;
    assert_eq!(composed.stale, 0);
    assert!(
        composed.updates + continued.coalesced
            >= resumed.compositing.updates + continued.animations,
        "composed={composed:?} resumed={:?} continued={continued:?}",
        resumed.compositing
    );
    evaluate(
        &mut worker,
        "document.getElementById('body').removeChild(pulse);",
    );
    let retired = worker.inspect();
    std::thread::sleep(Duration::from_millis(180));
    worker.pulse(graphics::shared::Pulse::now()).unwrap();
    worker.poll().unwrap();
    assert_eq!(worker.inspect().computation, retired.computation);
    assert_eq!(worker.inspect().compositing, retired.compositing);
    worker.close();
}

#[test]
fn held() {
    let mut worker = Worker::spawn(
        "app://animation/index.html",
        Extent {
            width: 32,
            height: 32,
        },
        Source,
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| {
        snapshot
            .execution
            .as_ref()
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == hold && job.phase() == Phase::Held)
    });
    let before = worker.inspect();
    for _ in 0..8 {
        worker.pulse(graphics::shared::Pulse::now()).unwrap();
        std::thread::sleep(Duration::from_millis(35));
    }
    worker.poll().unwrap();
    let after = worker.inspect();
    let computation = after.computation.since(before.computation);
    assert!(computation.animations >= 2);
    assert!(computation.coalesced >= 1);
    assert_eq!(after.compositing.updates, before.compositing.updates + 1);
    assert!(
        after
            .execution
            .as_ref()
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == hold && job.phase() == Phase::Held)
    );
    worker.unhold().unwrap();
    until(&mut worker, |snapshot| {
        snapshot
            .execution
            .as_ref()
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == hold && job.phase() == Phase::Complete)
    });
    worker.close();
}
