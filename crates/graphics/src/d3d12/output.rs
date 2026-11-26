use crate::d3d12::ffi::{Com, Event, check, constants, tables, types};
use crate::shared::{Extent, Fault, Image, State};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct Signal {
    pub device: Com,
    pub fence: Com,
    pub value: u64,
}

impl Signal {
    pub fn state(&self) -> State {
        // SAFETY: self.fence is a live ID3D12Fence whose GetCompletedValue is thread-safe.
        let done = unsafe {
            let table = self.fence.vtable::<tables::Fence>();
            (table.completed)(self.fence.raw())
        };
        if done == u64::MAX {
            return State::Failed(removed(&self.device));
        }
        if done >= self.value {
            State::Complete
        } else {
            State::Pending
        }
    }

    pub fn wait(&self, timeout: Duration) -> Result<(), Fault> {
        match self.state() {
            State::Complete => return Ok(()),
            State::Failed(fault) => return Err(fault),
            State::Pending => {}
        }
        let event = Event::new()?;
        // SAFETY: self.fence is live and event.raw() stays valid until WaitForSingleObject returns.
        let hr = unsafe {
            let table = self.fence.vtable::<tables::Fence>();
            (table.event)(self.fence.raw(), self.value, event.raw())
        };
        check(hr, "ID3D12Fence::SetEventOnCompletion")?;
        if !event.wait(timeout)? {
            return Err(Fault("D3D12 completion timed out".into()));
        }
        match self.state() {
            State::Complete => Ok(()),
            State::Failed(fault) => Err(fault),
            State::Pending => Err(Fault("D3D12 signalled incomplete work".into())),
        }
    }
}

pub(crate) fn removed(device: &Com) -> Fault {
    // SAFETY: device is a live ID3D12Device; GetDeviceRemovedReason takes no pointers.
    let reason = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.removed)(device.raw())
    };
    Fault(format!(
        "D3D12 device removed (HRESULT 0x{:08x})",
        reason as u32
    ))
}

struct Wire(Signal);
// SAFETY: ID3D12Fence::GetCompletedValue/SetEventOnCompletion and ID3D12Device::GetDeviceRemovedReason are free-threaded COM methods.
unsafe impl Send for Wire {}

#[derive(Clone)]
pub(crate) struct Completion {
    signal: Signal,
    gate: Option<Arc<(Mutex<bool>, Condvar)>>,
}

impl Completion {
    pub fn attach(signal: Signal, notify: Option<shared::Notify>) -> Self {
        let Some(notify) = notify else {
            return Self { signal, gate: None };
        };
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let sender = gate.clone();
        let wire = Wire(signal.clone());
        let spawned = std::thread::Builder::new()
            .name("d3d12-fence".into())
            .spawn(move || {
                let target = wire;
                let _ = target.0.wait(Duration::from_secs(60));
                *sender.0.lock().expect("completion lock") = true;
                sender.1.notify_all();
                notify.notify();
            })
            .is_ok();
        Self {
            signal,
            gate: spawned.then_some(gate),
        }
    }

    pub fn wait(&self, timeout: Duration) -> Result<(), Fault> {
        if let Some(gate) = &self.gate {
            let ready = gate.0.lock().expect("completion lock");
            let (ready, _) = gate
                .1
                .wait_timeout_while(ready, timeout, |done| !*done)
                .expect("completion lock");
            if !*ready {
                return Err(Fault("D3D12 completion timed out".into()));
            }
            return self.signal.wait(Duration::ZERO);
        }
        self.signal.wait(timeout)
    }
}

pub(crate) struct Receipt {
    pub signal: Signal,
    pub queue: Com,
    pub texture: Com,
    pub extent: Extent,
    pub stamps: Option<(Com, u64)>,
    pub retained: Vec<Com>,
}

// SAFETY: Every COM interface retained by Receipt is immutable after GPU submission;
// IUnknown::AddRef/Release, ID3D12Fence, ID3D12Device, and ID3D12CommandQueue are free-threaded,
// and Drop synchronizes on the submission fence before releasing retained objects.
unsafe impl Send for Receipt {}
// SAFETY: Shared references to Receipt only read immutable fields or invoke thread-safe COM queries.
unsafe impl Sync for Receipt {}

impl Drop for Receipt {
    fn drop(&mut self) {
        if self.signal.state() == State::Pending {
            let _ = self.signal.wait(Duration::from_secs(5));
        }
        self.retained.clear();
    }
}

pub(crate) fn commit(
    hardware: (&Com, &Com),
    list: &Com,
    fence: (&Com, &mut u64),
) -> Result<Signal, Fault> {
    let (device, queue) = hardware;
    // SAFETY: list is an open ID3D12GraphicsCommandList owned by this submission.
    let hr = unsafe {
        let table = list.vtable::<tables::List>();
        (table.close)(list.raw())
    };
    check(hr, "ID3D12GraphicsCommandList::Close")?;
    let raw = [list.raw()];
    // SAFETY: queue is a live direct ID3D12CommandQueue and raw points to one closed command list.
    unsafe {
        let table = queue.vtable::<tables::Queue>();
        (table.execute)(queue.raw(), 1, raw.as_ptr());
    }
    *fence.1 += 1;
    let value = *fence.1;
    // SAFETY: queue and fence belong to the same live ID3D12Device.
    let hr = unsafe {
        let table = queue.vtable::<tables::Queue>();
        (table.signal)(queue.raw(), fence.0.raw(), value)
    };
    check(hr, "ID3D12CommandQueue::Signal")?;
    Ok(Signal {
        device: device.clone(),
        fence: fence.0.clone(),
        value,
    })
}

pub(crate) fn read(
    hardware: (&Com, &Com),
    fence: (&Com, &mut u64),
    receipt: &Receipt,
) -> Result<Image, Fault> {
    if receipt.signal.state() != State::Complete {
        return Err(Fault("readback requires completed rendering".into()));
    }
    let (device, queue) = hardware;
    let stride = (receipt.extent.width * 4).div_ceil(256) * 256;
    let size = stride * receipt.extent.height;
    let allocator = crate::d3d12::memory::Allocator(device);
    let buffer = allocator.buffer(size, (constants::READBACK, constants::COPYDEST))?;
    let (pool, list) = allocator.command()?;
    let source = types::Location {
        resource: receipt.texture.raw(),
        kind: 0,
        data: types::Copy { index: 0 },
    };
    let dest = types::Location {
        resource: buffer.raw(),
        kind: 1,
        data: types::Copy {
            placed: types::Placed {
                offset: 0,
                footprint: types::Footprint {
                    format: constants::BGRA,
                    width: receipt.extent.width as u32,
                    height: receipt.extent.height as u32,
                    depth: 1,
                    pitch: stride as u32,
                },
            },
        },
    };
    // SAFETY: source is in COPY_SOURCE after completed rendering; dest is a READBACK buffer sized stride * height.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.copytexture)(list.raw(), &dest, 0, 0, 0, &source, std::ptr::null());
    }
    let signal = commit((device, queue), &list, fence)?;
    signal.wait(Duration::from_secs(5))?;
    let _keep = (pool, list);
    unpack(&buffer, receipt.extent, stride, size)
}

fn unpack(buffer: &Com, extent: Extent, stride: usize, size: usize) -> Result<Image, Fault> {
    let mut mapped = std::ptr::null_mut();
    let range = types::Range {
        begin: 0,
        end: size,
    };
    // SAFETY: buffer is a READBACK resource whose copy fence completed; range bounds the read.
    let hr = unsafe {
        let table = buffer.vtable::<tables::Resource>();
        (table.map)(buffer.raw(), 0, &range, &mut mapped)
    };
    check(hr, "ID3D12Resource::Map")?;
    // SAFETY: mapped points to size readable bytes for the lifetime of this mapping.
    let bytes = unsafe { std::slice::from_raw_parts(mapped as *const u8, size) };
    let mut pixels = Vec::with_capacity(extent.area()?);
    for row in bytes.chunks(stride) {
        for pixel in row[..extent.width * 4].chunks_exact(4) {
            pixels.push(
                u32::from(pixel[0]) | (u32::from(pixel[1]) << 8) | (u32::from(pixel[2]) << 16),
            );
        }
    }
    let empty = types::Range { begin: 0, end: 0 };
    // SAFETY: Matches the Map call above; zero written range signals CPU read-only access.
    unsafe {
        let table = buffer.vtable::<tables::Resource>();
        (table.unmap)(buffer.raw(), 0, &empty);
    }
    Ok(Image { extent, pixels })
}

pub(crate) fn elapsed(receipt: &Receipt) -> Option<Duration> {
    if receipt.signal.state() != State::Complete {
        return None;
    }
    let (buffer, frequency) = receipt.stamps.as_ref()?;
    if *frequency == 0 {
        return None;
    }
    let mut mapped = std::ptr::null_mut();
    let range = types::Range { begin: 0, end: 16 };
    // SAFETY: buffer is a 16-byte READBACK buffer written by ResolveQueryData before receipt.signal completed.
    let hr = unsafe {
        let table = buffer.vtable::<tables::Resource>();
        (table.map)(buffer.raw(), 0, &range, &mut mapped)
    };
    if hr < 0 || mapped.is_null() {
        return None;
    }
    // SAFETY: mapped holds two resolved u64 GPU timestamps.
    let pair = unsafe { *(mapped as *const [u64; 2]) };
    let empty = types::Range { begin: 0, end: 0 };
    // SAFETY: Unmaps the matching timestamp readback buffer.
    unsafe {
        let table = buffer.vtable::<tables::Resource>();
        (table.unmap)(buffer.raw(), 0, &empty);
    }
    (pair[1] >= pair[0]).then(|| {
        let nanos = u128::from(pair[1] - pair[0]) * 1_000_000_000 / u128::from(*frequency);
        Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
    })
}
