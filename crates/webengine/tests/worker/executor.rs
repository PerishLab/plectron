#![cfg(test)]

use super::*;
use domengine::style::animation::Schedule;
use domengine::style::{Animation, Keyframes, Timing, animation::Animations};

fn fixture() -> (domengine::Node, Keyframes, Animation) {
    let document = domengine::html::parse("<div id='pulse'></div>").unwrap();
    let node = document.find("pulse").unwrap();
    let frames = Keyframes::new("blink".into(), vec![(0.0, 255), (0.5, 0), (1.0, 255)]).unwrap();
    let animation = Animation {
        name: Some("blink".into()),
        duration: 1_200_000,
        iterations: None,
        timing: Timing::Steps(1, false),
        ..Animation::default()
    };
    (node, frames, animation)
}

fn mapped(animations: &Animations) -> Vec<Track<u64>> {
    animations
        .plans()
        .into_iter()
        .map(|track| Track {
            target: 7,
            plan: track.plan,
        })
        .collect()
}

fn tracks(playing: bool) -> Vec<Track<u64>> {
    let (node, frames, mut animation) = fixture();
    animation.playing = playing;
    let mut animations = Animations::default();
    animations.sample(node, &animation, &frames, 255);
    mapped(&animations)
}

fn sample(executor: &mut Executor, shared: &Mutex<Shared>, now: u64) {
    publish(shared, executor.animator.advance(now), 0, None).unwrap();
}

#[test]
fn coalescing() {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let mut executor = Executor::default();
    executor
        .replace(
            Replacement {
                generation: 1,
                tracks: tracks(true),
            },
            0,
            &shared,
        )
        .unwrap();
    sample(&mut executor, &shared, 600_000);
    sample(&mut executor, &shared, 1_200_000);
    assert_eq!(shared.lock().unwrap().statistics.coalesced, 1);
    let (sender, _receiver) = mpsc::sync_channel(CAPACITY);
    let reader = Reader {
        channel: Channel {
            sender,
            pending: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            shared: shared.clone(),
            clock: Instant::now(),
        },
    };
    assert_eq!(reader.take().unwrap().unwrap().generation(), 1);
    assert_eq!(shared.lock().unwrap().statistics.submitted, 1);
    sample(&mut executor, &shared, 1_800_000);
    executor
        .replace(
            Replacement {
                generation: 2,
                tracks: Vec::new(),
            },
            0,
            &shared,
        )
        .unwrap();
    let shared = shared.lock().unwrap();
    assert!(shared.effect.is_none());
    assert_eq!(shared.statistics.animations, 3);
    assert_eq!(shared.statistics.discarded, 1);
    assert_eq!(
        shared.statistics.animations,
        shared.statistics.submitted + shared.statistics.coalesced + shared.statistics.discarded
    );
}

#[test]
fn pausing() {
    let shared = Mutex::new(Shared::default());
    let mut executor = Executor::default();
    executor
        .replace(
            Replacement {
                generation: 1,
                tracks: tracks(false),
            },
            0,
            &shared,
        )
        .unwrap();
    assert_eq!(executor.animator.schedule(10_000_000), None);
    sample(&mut executor, &shared, 10_000_000);
    assert_eq!(shared.lock().unwrap().statistics.animations, 0);
    executor
        .replace(
            Replacement {
                generation: 2,
                tracks: Vec::new(),
            },
            900_000,
            &shared,
        )
        .unwrap();
    assert_eq!(executor.animator.schedule(10_000_000), None);
}

#[test]
fn resuming() {
    let (node, frames, mut animation) = fixture();
    let mut animations = Animations::default();
    animations.sample(node, &animation, &frames, 255);
    animations.advance(600_000);
    animations.changes();
    animation.playing = false;
    animations.sample(node, &animation, &frames, 255);

    let shared = Mutex::new(Shared::default());
    let mut executor = Executor::default();
    executor
        .replace(
            Replacement {
                generation: 1,
                tracks: mapped(&animations),
            },
            900_000,
            &shared,
        )
        .unwrap();
    assert_eq!(executor.animator.schedule(900_000), None);

    animations.advance(900_000);
    animation.playing = true;
    animations.sample(node, &animation, &frames, 255);
    executor
        .replace(
            Replacement {
                generation: 2,
                tracks: mapped(&animations),
            },
            900_000,
            &shared,
        )
        .unwrap();
    assert_eq!(
        executor.animator.schedule(900_000),
        Some(Schedule::At(1_500_000))
    );
    sample(&mut executor, &shared, 1_499_999);
    assert!(shared.lock().unwrap().effect.is_none());
    sample(&mut executor, &shared, 1_500_000);
    assert_eq!(
        shared.lock().unwrap().effect.as_ref().unwrap().generation(),
        2
    );
}

#[test]
fn retirement() {
    let shared = Mutex::new(Shared::default());
    let mut executor = Executor::default();
    executor
        .replace(
            Replacement {
                generation: 1,
                tracks: tracks(true),
            },
            0,
            &shared,
        )
        .unwrap();
    sample(&mut executor, &shared, 600_000);
    executor.shutdown(&shared).unwrap();
    assert_eq!(executor.animator.schedule(1_200_000), None);
    let shared = shared.lock().unwrap();
    assert!(shared.effect.is_none());
    assert_eq!(shared.statistics.discarded, 1);
}

#[test]
fn pacing() {
    let clock = Instant::now();
    let shared = Arc::new(Mutex::new(Shared {
        generation: 7,
        schedule: Some(Schedule::At(100)),
        ..Shared::default()
    }));
    let (sender, _receiver) = mpsc::sync_channel(CAPACITY);
    let reader = Reader {
        channel: Channel {
            sender,
            pending: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            shared: shared.clone(),
            clock,
        },
    };
    assert!(
        !reader
            .pulse(graphics::shared::Pulse::new(
                clock + std::time::Duration::from_micros(99)
            ))
            .unwrap()
    );
    assert!(shared.lock().unwrap().sample.is_none());
    assert!(
        reader
            .pulse(graphics::shared::Pulse::new(
                clock + std::time::Duration::from_micros(100)
            ))
            .unwrap()
    );
    let shared = shared.lock().unwrap();
    let sample = shared.sample.unwrap();
    assert_eq!((sample.generation, sample.now), (7, 100));
    assert_eq!(shared.statistics.pulses, 1);
}

#[test]
fn shutdown() {
    let (mut controller, reader) = spawn(Instant::now(), None).unwrap();
    controller.replace(1, tracks(true)).unwrap();
    controller.shutdown();
    let statistics = reader.statistics().unwrap();
    assert!(statistics.mailbox >= 1);
}
