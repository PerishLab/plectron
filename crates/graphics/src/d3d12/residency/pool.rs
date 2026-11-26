use crate::d3d12::ffi::Com;
use crate::d3d12::memory::Texture;
use crate::d3d12::output::Signal;
use crate::shared::{Extent, Fault, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub entries: usize,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub entries: usize,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub policy: Policy,
    pub closed: bool,
    pub idle: Usage,
    pub pending: Usage,
    pub checked: Usage,
    pub allocations: u64,
    pub allocated: u64,
    pub reuses: u64,
    pub retired: u64,
    pub pixels: u64,
}

struct Pending {
    signal: Signal,
    textures: Vec<Texture>,
    recycle: bool,
}

pub struct Pool {
    report: Snapshot,
    idle: Vec<Texture>,
    checked: Vec<Texture>,
    pending: Vec<Pending>,
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            report: Snapshot {
                policy: Policy {
                    entries: 32,
                    bytes: 33_554_432,
                },
                closed: false,
                idle: Usage::default(),
                pending: Usage::default(),
                checked: Usage::default(),
                allocations: 0,
                allocated: 0,
                reuses: 0,
                retired: 0,
                pixels: 0,
            },
            idle: Vec::new(),
            checked: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl Pool {
    pub fn inspect(&mut self) -> Snapshot {
        self.maintain();
        let mut report = self.report;
        report.idle = usage(self.idle.iter());
        report.checked = usage(self.checked.iter());
        report.pending = usage(self.pending.iter().flat_map(|entry| &entry.textures));
        report
    }

    pub fn configure(&mut self, policy: Policy) -> Result<(), Fault> {
        self.open()?;
        self.report.policy = policy;
        self.trim();
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), Fault> {
        self.open()?;
        self.report.retired += self.idle.len() as u64;
        self.idle.clear();
        self.discard();
        for entry in &mut self.pending {
            entry.recycle = false;
        }
        Ok(())
    }

    pub fn destroy(&mut self) -> Result<(), Fault> {
        self.clear()?;
        self.report.closed = true;
        Ok(())
    }

    fn open(&self) -> Result<(), Fault> {
        if self.report.closed {
            return Err(Fault("texture pool destroyed".into()));
        }
        Ok(())
    }

    pub(crate) fn acquire(&mut self, device: &Com, extent: Extent) -> Result<Texture, Fault> {
        let found = self
            .idle
            .iter()
            .rposition(|texture| texture.extent == extent && texture.device == *device);
        let texture = match found {
            Some(index) => {
                self.report.reuses += 1;
                self.idle.swap_remove(index)
            }
            None => {
                let texture = crate::d3d12::memory::Allocator(device).texture(extent)?;
                self.report.allocations += 1;
                self.report.allocated += texture.allocated as u64;
                texture
            }
        };
        self.report.pixels += extent.area()? as u64;
        self.checked.push(texture.clone());
        Ok(texture)
    }

    pub(crate) fn submit(&mut self, signal: Signal) {
        if self.checked.is_empty() {
            return;
        }
        self.pending.push(Pending {
            signal,
            textures: std::mem::take(&mut self.checked),
            recycle: !self.report.closed,
        });
    }

    pub(crate) fn discard(&mut self) {
        self.report.retired += self.checked.len() as u64;
        self.checked.clear();
    }

    pub(crate) fn maintain(&mut self) {
        let mut index = 0;
        while index < self.pending.len() {
            let state = self.pending[index].signal.state();
            if state == State::Pending {
                index += 1;
                continue;
            }
            let entry = self.pending.swap_remove(index);
            if state == State::Complete && entry.recycle {
                self.idle.extend(entry.textures);
            } else {
                self.report.retired += entry.textures.len() as u64;
            }
        }
        self.trim();
    }

    fn trim(&mut self) {
        let mut held = usage(self.idle.iter());
        while held.entries > self.report.policy.entries || held.bytes > self.report.policy.bytes {
            let texture = self.idle.remove(0);
            held.entries -= 1;
            held.bytes -= texture.allocated;
            self.report.retired += 1;
        }
    }
}

fn usage<'a>(textures: impl Iterator<Item = &'a Texture>) -> Usage {
    textures.fold(Usage::default(), |mut total, texture| {
        total.entries += 1;
        total.bytes += texture.allocated;
        total
    })
}
