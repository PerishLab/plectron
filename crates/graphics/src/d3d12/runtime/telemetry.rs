use crate::d3d12::ffi::{self, Com, constants, tables, types};
use crate::shared::Visibility as Evidence;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(super) struct Telemetry {
    chain: Com,
    target: u32,
    baseline: Option<u64>,
    completed: Instant,
    cached: Mutex<Option<Evidence>>,
}

// SAFETY: IDXGISwapChain::GetFrameStatistics is a read-only DXGI query; Telemetry is dropped
// inside Worker::drop before Surface::drop returns so no swapchain reference outlives teardown.
unsafe impl Send for Telemetry {}

impl Telemetry {
    pub fn capture(chain: &Com, completed: Instant) -> Option<Self> {
        let mut target = 0u32;
        // SAFETY: chain is a live IDXGISwapChain3 and target is a writable u32 out parameter.
        let hr = unsafe {
            let table = chain.vtable::<tables::Chain>();
            (table.count)(chain.raw(), &mut target)
        };
        let baseline = ffi::dwm().map(|(count, _)| count);
        if (hr < 0 || target == 0) && baseline.is_none() {
            return None;
        }
        let mut stats = types::Stats::default();
        // SAFETY: Priming GetFrameStatistics clears any initial DISJOINT status on new swapchains.
        let _ = unsafe {
            let table = chain.vtable::<tables::Chain>();
            (table.stats)(chain.raw(), &mut stats)
        };
        Some(Self {
            chain: chain.clone(),
            target: if hr >= 0 { target } else { 0 },
            baseline,
            completed,
            cached: Mutex::new(None),
        })
    }

    pub fn evidence(&self) -> Evidence {
        let mut guard = self.cached.lock().expect("visibility lock");
        if let Some(evidence) = *guard {
            return evidence;
        }
        if let Some(evidence) = self.dxgi() {
            if !matches!(evidence, Evidence::Pending) {
                *guard = Some(evidence);
            }
            return evidence;
        }
        let evidence = self.composition();
        if !matches!(evidence, Evidence::Pending) {
            *guard = Some(evidence);
        }
        evidence
    }

    fn dxgi(&self) -> Option<Evidence> {
        if self.target == 0 {
            return None;
        }
        let mut stats = types::Stats::default();
        // SAFETY: self.chain is a live IDXGISwapChain3 and stats is writable.
        let hr = unsafe {
            let table = self.chain.vtable::<tables::Chain>();
            (table.stats)(self.chain.raw(), &mut stats)
        };
        if hr < 0 || hr == constants::DISJOINT {
            return None;
        }
        if stats.present == 0 {
            return None;
        }
        if stats.present < self.target {
            return Some(Evidence::Pending);
        }
        if stats.present > self.target {
            return Some(Evidence::Dropped);
        }
        let visible = project(stats.qpc, self.completed)
            .unwrap_or_else(|| Instant::now().max(self.completed));
        Some(Evidence::Visible(visible))
    }

    fn composition(&self) -> Evidence {
        let Some(baseline) = self.baseline else {
            return Evidence::Unobserved;
        };
        let Some((count, vblank)) = ffi::dwm() else {
            return Evidence::Unobserved;
        };
        if count <= baseline {
            return Evidence::Pending;
        }
        let visible = project(vblank as i64, self.completed)
            .unwrap_or_else(|| Instant::now().max(self.completed));
        Evidence::Visible(visible)
    }
}

fn project(qpc: i64, completed: Instant) -> Option<Instant> {
    if qpc <= 0 {
        return None;
    }
    let (now, frequency) = ffi::qpc()?;
    let observed = Instant::now();
    let sample = qpc as u64;
    let delta = now.abs_diff(sample);
    let nanos = (u128::from(delta) * 1_000_000_000) / u128::from(frequency);
    if nanos > 10_000_000_000 {
        return None;
    }
    let span = Duration::from_nanos(nanos as u64);
    let instant = if now >= sample {
        observed.checked_sub(span)?
    } else {
        observed.checked_add(span)?
    };
    Some(instant.max(completed).min(observed))
}

pub fn refresh() -> Option<u64> {
    ffi::dwm().map(|(count, _)| count)
}

pub fn settled(baseline: u64, completed: Instant) -> Option<Instant> {
    let (count, vblank) = ffi::dwm()?;
    if count <= baseline {
        return None;
    }
    Some(project(vblank as i64, completed).unwrap_or_else(|| Instant::now().max(completed)))
}
