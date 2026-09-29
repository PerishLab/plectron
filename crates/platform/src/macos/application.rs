use super::Error;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

pub(super) struct Application {
    native: Retained<NSApplication>,
}

impl Application {
    pub(super) fn new() -> Result<Self, Error> {
        let marker = MainThreadMarker::new()
            .ok_or_else(|| Error("application identity requires main thread".into()))?;
        let application = NSApplication::sharedApplication(marker);
        if !application.setActivationPolicy(NSApplicationActivationPolicy::Regular) {
            return Err(Error("application identity was rejected".into()));
        }
        application.finishLaunching();
        Ok(Self {
            native: application,
        })
    }

    pub fn activate(&self) {
        self.native.activate();
    }

    pub fn poll(&self) {
        unsafe extern "C" {
            fn plectron_application_poll();
        }
        // SAFETY: AppKit events are drained synchronously on the platform main thread.
        unsafe { plectron_application_poll() };
    }
}
