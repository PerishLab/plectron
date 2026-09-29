use super::Error;
use graphics::shared::Pacing;
use objc2_core_foundation::{
    CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFRunLoopCommonModes,
    kCFRunLoopDefaultMode,
};
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;

struct Signal {
    runloop: CFRetained<CFRunLoop>,
    source: CFRetained<CFRunLoopSource>,
}

// SAFETY: Core Foundation explicitly permits signalling a run-loop source and
// waking its run loop from another thread. All other operations stay on main.
unsafe impl Send for Signal {}
unsafe impl Sync for Signal {}

pub(super) struct Events {
    signal: Arc<Signal>,
}

impl Events {
    pub fn new() -> Result<Self, Error> {
        let runloop = CFRunLoop::main().ok_or_else(|| Error("main run loop unavailable".into()))?;
        let mut context = CFRunLoopSourceContext {
            version: 0,
            info: std::ptr::null_mut(),
            retain: None,
            release: None,
            copyDescription: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform: Some(perform),
        };
        let source = unsafe { CFRunLoopSource::new(None, 0, &mut context) }
            .ok_or_else(|| Error("run loop source unavailable".into()))?;
        runloop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        Ok(Self {
            signal: Arc::new(Signal { runloop, source }),
        })
    }

    pub fn notifier(&self) -> shared::Notify {
        let signal = self.signal.clone();
        shared::Notify::new(move || {
            signal.source.signal();
            signal.runloop.wake_up();
        })
    }

    pub fn wait(&self, pacing: Pacing) {
        let seconds = match pacing {
            Pacing::At(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64(),
            Pacing::Idle | Pacing::Continuous => 86_400.0,
        };
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, seconds, true);
    }
}

impl Drop for Events {
    fn drop(&mut self) {
        self.signal
            .runloop
            .remove_source(Some(&self.signal.source), unsafe { kCFRunLoopCommonModes });
        self.signal.source.invalidate();
    }
}

extern "C-unwind" fn perform(info: *mut c_void) {
    let _ = info;
}

#[path = "../../tests/macos/event.rs"]
mod checks;
