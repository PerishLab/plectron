use crate::metal::{Work, output};
use crate::shared::{Fault, State, Visibility as Evidence};
use objc2::{
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_metal::*;
use objc2_quartz_core::{CACurrentMediaTime, CAMetalDrawable, CAMetalLayer};
use shared::budget::Budget;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

static WORKERS: Budget = Budget::new(8);

struct Layer(Retained<CAMetalLayer>);
// SAFETY: Only this private owner acquires drawables on one rendering thread, as
// supported by CAMetalLayer. Configuration is frozen before transfer; AppKit view
// access and layer attachment/removal stay on the main thread. Retention keeps the
// detached layer alive until this owner stops. No mutable Rust reference is shared.
unsafe impl Send for Layer {}

struct Job(Work);
// SAFETY: Work retains an immutable texture and an already completed command.
// The worker only reads that texture and creates its own command buffer through
// the thread-safe Metal command queue. Encoding never shares a command or encoder.
unsafe impl Send for Job {}

struct Outcome {
    result: Result<(), Fault>,
    completed: std::time::Instant,
    visibility: Option<Visibility>,
}

pub(crate) struct Worker {
    sender: SyncSender<Job>,
    receiver: Receiver<Outcome>,
    cancel: Arc<AtomicBool>,
    pending: bool,
    completed: Option<std::time::Instant>,
    visibility: Option<Visibility>,
}

impl Worker {
    pub fn new(
        layer: Retained<CAMetalLayer>,
        notify: Option<shared::Notify>,
        require_visibility: bool,
    ) -> Result<Self, Fault> {
        let permit = WORKERS
            .acquire()
            .ok_or_else(|| Fault("presentation worker budget exhausted".into()))?;
        let (sender, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let layer = Layer(layer);
        std::thread::Builder::new()
            .name("presentation".into())
            .spawn(move || {
                let _permit = permit;
                layer.run(jobs, results, (stop, notify, require_visibility));
            })
            .map_err(crate::metal::error)?;
        Ok(Self {
            sender,
            receiver,
            cancel,
            pending: false,
            completed: None,
            visibility: None,
        })
    }
    pub fn present(&mut self, work: &Work) -> Result<(), Fault> {
        if self.pending {
            return Err(Fault("presentation already pending".into()));
        }
        if output::state(&work.command) != State::Complete {
            return Err(Fault("presentation requires completed rendering".into()));
        }
        self.sender
            .try_send(Job(work.clone()))
            .map_err(crate::metal::error)?;
        self.pending = true;
        self.completed = None;
        self.visibility = None;
        Ok(())
    }
    pub fn completed(&self) -> Option<std::time::Instant> {
        self.completed
    }
    pub fn visibility(&self) -> Evidence {
        self.visibility
            .as_ref()
            .map_or(Evidence::Unobserved, Visibility::evidence)
    }
    pub fn poll(&mut self) -> Result<State, Fault> {
        if !self.pending {
            return Err(Fault("presentation is absent".into()));
        }
        match self.receiver.try_recv() {
            Ok(outcome) => {
                self.completed = Some(outcome.completed);
                self.visibility = outcome.visibility;
                self.pending = false;
                outcome.result.map(|()| State::Complete)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(State::Pending),
            Err(fault) => Err(crate::metal::error(fault)),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl Layer {
    fn run(
        self,
        jobs: Receiver<Job>,
        results: SyncSender<Outcome>,
        options: (Arc<AtomicBool>, Option<shared::Notify>, bool),
    ) {
        let (stop, notify, require_visibility) = options;
        while let Ok(job) = jobs.recv() {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let outcome = autoreleasepool(|_| self.draw(&job.0, &stop, require_visibility));
            if stop.load(Ordering::Acquire) || results.try_send(outcome).is_err() {
                break;
            }
            if let Some(notify) = &notify {
                notify.notify();
            }
        }
    }
    fn draw(&self, work: &Work, stop: &AtomicBool, require_visibility: bool) -> Outcome {
        let result = self.encode(work, stop, require_visibility);
        match result {
            Ok((completed, visibility)) => Outcome {
                result: Ok(()),
                completed,
                visibility,
            },
            Err(fault) => Outcome {
                result: Err(fault),
                completed: std::time::Instant::now(),
                visibility: None,
            },
        }
    }
    fn encode(
        &self,
        work: &Work,
        stop: &AtomicBool,
        require_visibility: bool,
    ) -> Result<(std::time::Instant, Option<Visibility>), Fault> {
        let drawable = self
            .0
            .nextDrawable()
            .ok_or_else(|| Fault("drawable unavailable".into()))?;
        if stop.load(Ordering::Acquire) {
            return Err(Fault("surface closed".into()));
        }
        let texture = drawable.texture();
        let command = copy(work, &texture)?;
        let visibility = require_visibility.then(|| Visibility::attach(&drawable));
        command.presentDrawable(ProtocolObject::from_ref(&*drawable));
        output::commit(&command)?;
        let completed = std::time::Instant::now();
        Ok((completed, visibility))
    }
}

#[derive(Clone)]
struct Visibility(Arc<Mutex<Option<Option<std::time::Instant>>>>);

impl Visibility {
    fn attach(drawable: &ProtocolObject<dyn CAMetalDrawable>) -> Self {
        let state = Arc::new(Mutex::new(None));
        let sender = state.clone();
        let callback = block2::RcBlock::new(
            move |drawable: std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>| {
                // SAFETY: Metal supplies a live drawable to this callback and keeps it
                // retained for the duration of the call.
                let drawable = unsafe { drawable.as_ref() };
                let presented = drawable.presentedTime();
                *sender.lock().expect("visibility lock") = Some(instant(presented));
            },
        );
        // SAFETY: Registered before presentation; Metal copies the live block. It
        // captures only thread-safe timing state and does not retain engine state.
        unsafe {
            drawable.addPresentedHandler(block2::RcBlock::as_ptr(&callback));
        }
        Self(state)
    }
    fn evidence(&self) -> Evidence {
        match *self.0.lock().expect("visibility lock") {
            None => Evidence::Pending,
            Some(None) => Evidence::Dropped,
            Some(Some(visible)) => Evidence::Visible(visible),
        }
    }
}

fn instant(presented: f64) -> Option<std::time::Instant> {
    if !presented.is_finite() || presented <= 0.0 {
        return None;
    }
    let media = CACurrentMediaTime();
    let latency = media - presented;
    if !latency.is_finite() || latency < 0.0 {
        return None;
    }
    std::time::Instant::now().checked_sub(std::time::Duration::from_secs_f64(latency))
}

pub(crate) fn copy(
    work: &Work,
    texture: &ProtocolObject<dyn MTLTexture>,
) -> Result<Retained<ProtocolObject<dyn MTLCommandBuffer>>, Fault> {
    let extent = crate::shared::Extent {
        width: texture.width(),
        height: texture.height(),
    };
    if extent != work.extent
        || *texture.device() != *work.texture.device()
        || texture.pixelFormat() != MTLPixelFormat::BGRA8Unorm
    {
        return Err(Fault("drawable extent, device or format changed".into()));
    }
    let command = work
        .queue
        .commandBuffer()
        .ok_or_else(|| Fault("presentation command allocation failed".into()))?;
    let encoder = command
        .blitCommandEncoder()
        .ok_or_else(|| Fault("presentation encoder allocation failed".into()))?;
    // SAFETY: Both textures belong to the same device and have equal BGRA8 dimensions.
    // The caller owns the destination and supplies completed immutable source work.
    // The command retains resources and orders this copy before presentation.
    unsafe {
        encoder.copyFromTexture_toTexture(&work.texture, texture);
    }
    encoder.endEncoding();
    Ok(command)
}
