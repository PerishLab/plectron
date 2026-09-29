use super::Animator;
use crate::Fault;
use domengine::style::animation::Track;
use shared::budget::Budget;
use std::sync::{Arc, Mutex, atomic::Ordering, mpsc};
use std::time::Instant;

mod channel;
pub(crate) use channel::Reader;
use channel::{Channel, Sample, Shared};

static EXECUTORS: Budget = Budget::new(8);
const CAPACITY: usize = 2;

struct Replacement {
    generation: u64,
    tracks: Vec<Track<u64>>,
}

enum Command {
    Replace(Replacement),
    Pulse,
    Shutdown,
}

pub(crate) struct Controller {
    channel: Option<Channel>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Controller {
    pub fn replace(&self, generation: u64, tracks: Vec<Track<u64>>) -> Result<(), Fault> {
        self.channel
            .as_ref()
            .ok_or_else(|| Fault("animation executor closed".into()))?
            .send(Command::Replace(Replacement { generation, tracks }))
    }

    pub fn shutdown(&mut self) {
        if let Some(channel) = self.channel.take() {
            let _ = channel.send(Command::Shutdown);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn spawn(
    clock: Instant,
    notify: Option<shared::Notify>,
) -> Result<(Controller, Reader), Fault> {
    let permit = EXECUTORS
        .acquire()
        .ok_or_else(|| Fault("animation executor budget exhausted".into()))?;
    let (sender, receiver) = mpsc::sync_channel(CAPACITY);
    let shared = Arc::new(Mutex::new(Shared::default()));
    let pending = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let channel = Channel {
        sender,
        pending,
        shared,
        clock,
    };
    let context = Context {
        receiver,
        channel: channel.clone(),
        notify,
    };
    let thread = std::thread::Builder::new()
        .name("plectron-animation".into())
        .spawn(move || {
            let _permit = permit;
            let _ = run(context);
        })
        .map_err(crate::error)?;
    Ok((
        Controller {
            channel: Some(channel.clone()),
            thread: Some(thread),
        },
        Reader { channel },
    ))
}

struct Context {
    receiver: mpsc::Receiver<Command>,
    channel: Channel,
    notify: Option<shared::Notify>,
}

fn run(context: Context) -> Result<(), ()> {
    let mut executor = Executor::default();
    loop {
        let Ok(command) = context.receiver.recv() else {
            return Ok(());
        };
        context.channel.pending.fetch_sub(1, Ordering::AcqRel);
        let now = elapsed(context.channel.clock);
        let changed = executor.control(command, now, &context.channel.shared)?;
        executor.sample(&context)?;
        if changed && let Some(notify) = &context.notify {
            notify.notify();
        }
    }
}

#[derive(Default)]
struct Executor {
    animator: Animator,
    generation: u64,
}

impl Executor {
    fn control(&mut self, command: Command, now: u64, shared: &Mutex<Shared>) -> Result<bool, ()> {
        match command {
            Command::Replace(replacement) => {
                self.replace(replacement, now, shared)?;
                Ok(true)
            }
            Command::Pulse => Ok(false),
            Command::Shutdown => {
                self.shutdown(shared)?;
                Err(())
            }
        }
    }

    fn replace(
        &mut self,
        replacement: Replacement,
        now: u64,
        shared: &Mutex<Shared>,
    ) -> Result<(), ()> {
        self.generation = replacement.generation;
        self.animator
            .replace(replacement.generation, replacement.tracks);
        let Ok(mut shared) = shared.lock() else {
            return Err(());
        };
        if shared.effect.take().is_some() {
            shared.statistics.discarded = shared.statistics.discarded.saturating_add(1);
        }
        shared.sample = None;
        shared.marker = false;
        shared.generation = self.generation;
        shared.schedule = self.animator.schedule(now);
        Ok(())
    }

    fn sample(&mut self, context: &Context) -> Result<(), ()> {
        let sample = {
            let Ok(mut shared) = context.channel.shared.lock() else {
                return Err(());
            };
            shared.marker = false;
            shared.sample.take()
        };
        let Some(Sample { generation, now }) = sample else {
            return Ok(());
        };
        if generation != self.generation {
            return Ok(());
        }
        let start = Instant::now();
        let result = self.animator.advance(now);
        let sampling = crate::page::elapsed(start);
        if let Ok(mut shared) = context.channel.shared.lock() {
            shared.schedule = self.animator.schedule(now);
        } else {
            return Err(());
        }
        publish(
            &context.channel.shared,
            result,
            sampling,
            context.notify.as_ref(),
        )
    }

    fn shutdown(&mut self, shared: &Mutex<Shared>) -> Result<(), ()> {
        self.animator = Animator::default();
        let Ok(mut shared) = shared.lock() else {
            return Err(());
        };
        if shared.effect.take().is_some() {
            shared.statistics.discarded = shared.statistics.discarded.saturating_add(1);
        }
        shared.schedule = None;
        shared.sample = None;
        shared.marker = false;
        Ok(())
    }
}

fn publish(
    shared: &Mutex<Shared>,
    result: Result<Option<graphics::shared::Effects>, graphics::shared::Fault>,
    sampling: u64,
    notify: Option<&shared::Notify>,
) -> Result<(), ()> {
    let Ok(mut shared) = shared.lock() else {
        return Err(());
    };
    shared.statistics.sampling = shared.statistics.sampling.saturating_add(sampling);
    let success = result.is_ok();
    match result {
        Ok(Some(effect)) => {
            shared.statistics.animations = shared.statistics.animations.saturating_add(1);
            shared.statistics.effects = shared
                .statistics
                .effects
                .saturating_add(effect.values().len() as u64);
            if shared.effect.replace(effect).is_some() {
                shared.statistics.coalesced = shared.statistics.coalesced.saturating_add(1);
            }
        }
        Ok(None) => return Ok(()),
        Err(fault) => {
            if shared.effect.take().is_some() {
                shared.statistics.discarded = shared.statistics.discarded.saturating_add(1);
            }
            shared.fault = Some(fault.to_string());
        }
    }
    drop(shared);
    if let Some(notify) = notify {
        notify.notify();
    }
    success.then_some(()).ok_or(())
}

fn elapsed(clock: Instant) -> u64 {
    clock.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

#[path = "../../../tests/worker/executor.rs"]
mod checks;
