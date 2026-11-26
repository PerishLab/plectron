use graphics::shared::{Pacing, Pulse};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Graphics::Dwm::{DWM_TIMING_INFO, DwmFlush, DwmGetCompositionTimingInfo},
    System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
};

const FALLBACK: Duration = Duration::from_nanos(16_666_667);

#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub refresh: u64,
    pub vblank: Instant,
    pub target: Instant,
}

struct Shared {
    active: bool,
    stopped: bool,
    native: bool,
    latest: Option<Sample>,
}

pub(super) struct Timing {
    shared: Arc<(Mutex<Shared>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl Timing {
    pub fn new(notify: shared::Notify) -> Self {
        let initial = snapshot(None);
        let native = initial.is_some();
        let shared = Arc::new((
            Mutex::new(Shared {
                active: false,
                stopped: false,
                native,
                latest: initial,
            }),
            Condvar::new(),
        ));
        let worker = if native {
            let state = shared.clone();
            std::thread::Builder::new()
                .name("dwm-pacer".into())
                .spawn(move || pacer(state, notify))
                .ok()
        } else {
            None
        };
        if native
            && worker.is_none()
            && let Ok(mut guard) = shared.0.lock()
        {
            guard.native = false;
        }
        Self { shared, worker }
    }

    #[cfg(test)]
    pub fn timer() -> Self {
        Self {
            shared: Arc::new((
                Mutex::new(Shared {
                    active: false,
                    stopped: true,
                    native: false,
                    latest: None,
                }),
                Condvar::new(),
            )),
            worker: None,
        }
    }

    pub fn native(&self) -> bool {
        self.shared
            .0
            .lock()
            .map(|guard| guard.native)
            .unwrap_or(false)
    }

    pub fn engage(&self, active: bool) {
        let Ok(mut guard) = self.shared.0.lock() else {
            return;
        };
        let next = active && guard.native && !guard.stopped;
        if guard.active != next {
            guard.active = next;
            if next {
                self.shared.1.notify_all();
            }
        }
    }

    pub fn latest(&self) -> Option<Sample> {
        self.shared.0.lock().ok().and_then(|guard| guard.latest)
    }

    pub fn current(&self) -> Option<Sample> {
        if !self.native() {
            return None;
        }
        let prior = self.latest();
        snapshot(prior).or(prior)
    }

    pub fn pulse(&self, seen: &mut u64, pacing: Pacing) -> Option<Pulse> {
        if !self.native() {
            return due(pacing).then(Pulse::now);
        }
        let sample = self.latest()?;
        if sample.refresh == *seen {
            return None;
        }
        *seen = sample.refresh;
        due(pacing).then(|| Pulse::new(sample.target))
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.shared.0.lock() {
            guard.stopped = true;
            guard.active = false;
            self.shared.1.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn pacer(shared: Arc<(Mutex<Shared>, Condvar)>, notify: shared::Notify) {
    while let Some(prior) = awaitable(&shared) {
        // SAFETY: DwmFlush takes no pointers and safely blocks this background thread until the next DWM composition pass.
        if unsafe { DwmFlush() } < 0 {
            if let Ok(mut guard) = shared.0.lock() {
                guard.native = false;
                guard.active = false;
            }
            notify.notify();
            return;
        }
        let observed = Instant::now();
        let next = snapshot(prior).unwrap_or_else(|| fallback(prior, observed));
        if let Ok(mut guard) = shared.0.lock() {
            guard.latest = Some(merge(guard.latest, next));
        }
        notify.notify();
    }
}

fn awaitable(shared: &(Mutex<Shared>, Condvar)) -> Option<Option<Sample>> {
    let mut guard = shared.0.lock().ok()?;
    while !guard.active && !guard.stopped {
        guard = shared.1.wait(guard).ok()?;
    }
    (!guard.stopped).then_some(guard.latest)
}

fn snapshot(prior: Option<Sample>) -> Option<Sample> {
    let mut info = DWM_TIMING_INFO {
        cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    // SAFETY: null HWND requests desktop composition timing on Windows 8+; info is writable with cbSize set.
    if unsafe { DwmGetCompositionTimingInfo(std::ptr::null_mut(), &mut info) } < 0 {
        return None;
    }
    let (now, frequency) = clock()?;
    let observed = Instant::now();
    let vblank = project(info.qpcVBlank, now, frequency, observed)
        .unwrap_or(observed)
        .min(observed);
    let period = interval(info.qpcRefreshPeriod, frequency).unwrap_or(FALLBACK);
    let target = vblank + period;
    let refresh = info.cRefresh.max(1);
    Some(merge(
        prior,
        Sample {
            refresh,
            vblank,
            target,
        },
    ))
}

fn fallback(prior: Option<Sample>, observed: Instant) -> Sample {
    let refresh = prior.map_or(1, |sample| sample.refresh.saturating_add(1));
    merge(
        prior,
        Sample {
            refresh,
            vblank: observed,
            target: observed + FALLBACK,
        },
    )
}

fn merge(prior: Option<Sample>, next: Sample) -> Sample {
    let Some(prior) = prior else {
        return next;
    };
    let refresh = next.refresh.max(prior.refresh);
    let vblank = next.vblank.max(prior.vblank);
    let target = next.target.max(prior.target);
    Sample {
        refresh,
        vblank,
        target,
    }
}

fn clock() -> Option<(u64, u64)> {
    let mut counter = 0i64;
    let mut frequency = 0i64;
    // SAFETY: counter and frequency are valid writable i64 outputs for QPC queries.
    let ok = unsafe {
        QueryPerformanceCounter(&mut counter) != 0 && QueryPerformanceFrequency(&mut frequency) != 0
    };
    (ok && counter >= 0 && frequency > 0).then_some((counter as u64, frequency as u64))
}

fn interval(ticks: u64, frequency: u64) -> Option<Duration> {
    if ticks == 0 || frequency == 0 {
        return None;
    }
    let nanos = (u128::from(ticks) * 1_000_000_000) / u128::from(frequency);
    (1_000_000..=1_000_000_000)
        .contains(&nanos)
        .then(|| Duration::from_nanos(nanos as u64))
}

fn project(qpc: u64, now: u64, frequency: u64, observed: Instant) -> Option<Instant> {
    if qpc == 0 || frequency == 0 {
        return None;
    }
    let delta = now.abs_diff(qpc);
    let nanos = (u128::from(delta) * 1_000_000_000) / u128::from(frequency);
    if nanos > 1_000_000_000 {
        return None;
    }
    let span = Duration::from_nanos(nanos as u64);
    if now >= qpc {
        observed.checked_sub(span)
    } else {
        observed.checked_add(span)
    }
}

pub(super) fn due(pacing: Pacing) -> bool {
    match pacing {
        Pacing::Continuous => true,
        Pacing::At(deadline) => Instant::now() >= deadline,
        Pacing::Idle => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection() {
        let observed = Instant::now();
        let before = project(9_950_000, 10_000_000, 10_000_000, observed).unwrap();
        assert_eq!(observed - before, Duration::from_millis(5));
        assert!(project(0, 10_000_000, 10_000_000, observed).is_none());
        assert_eq!(
            interval(166_667, 10_000_000).unwrap(),
            Duration::from_nanos(16_666_700)
        );
        assert!(interval(0, 10_000_000).is_none());
        let timer = Timing::timer();
        assert!(!timer.native());
        let mut seen = 0;
        assert!(timer.pulse(&mut seen, Pacing::Idle).is_none());
        assert!(timer.pulse(&mut seen, Pacing::Continuous).is_some());
    }
}
