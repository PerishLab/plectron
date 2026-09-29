use super::{Error, window::Window};
use graphics::shared::Pacing;
use graphics::shared::Pulse;
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, msg_send, sel};
use objc2_foundation::{NSObject, NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink};
use std::cell::Cell;
use std::time::{Duration, Instant};

struct Ivars {
    clock: Instant,
    media: f64,
    pulse: Cell<Option<Instant>>,
}

objc2::define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        #[unsafe(method(tick:))]
        fn tick(&self, link: &CADisplayLink) {
            if let Some(target) = instant(
                self.ivars().clock,
                self.ivars().media,
                link.targetTimestamp(),
            ) {
                self.ivars().pulse.set(Some(target));
            }
        }
    }
);

impl Target {
    fn new(marker: MainThreadMarker) -> Retained<Self> {
        let media = CACurrentMediaTime();
        let clock = Instant::now();
        let this = Self::alloc(marker).set_ivars(Ivars {
            clock,
            media,
            pulse: Cell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub(super) struct Timing {
    target: Retained<Target>,
    link: Option<Retained<CADisplayLink>>,
    pacing: Cell<Pacing>,
}

impl Timing {
    pub fn new(window: &Window) -> Result<Self, Error> {
        let marker = MainThreadMarker::new()
            .ok_or_else(|| Error("display pacing requires main thread".into()))?;
        let target = Target::new(marker);
        let view = window.view();
        if !view.respondsToSelector(sel!(displayLinkWithTarget:selector:)) {
            return Ok(Self {
                target,
                link: None,
                pacing: Cell::new(Pacing::Idle),
            });
        }
        let link = unsafe { view.displayLinkWithTarget_selector(&target, sel!(tick:)) };
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        link.setPaused(true);
        Ok(Self {
            target,
            link: Some(link),
            pacing: Cell::new(Pacing::Idle),
        })
    }

    pub fn pulse(&self) -> Option<Pulse> {
        match &self.link {
            Some(_) => self.target.ivars().pulse.take().map(Pulse::new),
            None if self.due() => Some(Pulse::now()),
            None => None,
        }
    }

    pub fn pace(&self, pacing: Pacing) {
        self.pacing.set(pacing);
        if let Some(link) = &self.link {
            link.setPaused(!matches!(pacing, Pacing::Continuous) && !self.due());
        }
    }

    pub fn native(&self) -> bool {
        self.link.is_some()
    }

    pub fn active(&self) -> bool {
        self.native() && self.due()
    }

    fn due(&self) -> bool {
        match self.pacing.get() {
            Pacing::Continuous => true,
            Pacing::At(deadline) => Instant::now() >= deadline,
            Pacing::Idle => false,
        }
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        if let Some(link) = self.link.take() {
            link.invalidate();
        }
    }
}

fn instant(clock: Instant, media: f64, target: f64) -> Option<Instant> {
    let duration = Duration::try_from_secs_f64(target - media).ok()?;
    clock.checked_add(duration)
}

#[path = "../../tests/macos/timing.rs"]
mod checks;
