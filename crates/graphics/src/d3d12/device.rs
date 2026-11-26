use crate::d3d12::ffi::{Com, adapter, check, constants, tables, types};
use crate::d3d12::{drawing, memory, output};
use crate::shared::{Backend, Fault, Image, Pass, State};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub struct D3d12 {
    notify: Option<shared::Notify>,
    pub(crate) device: Com,
    pub(crate) queue: Com,
    pub(crate) fence: Com,
    pub(crate) value: u64,
    frequency: u64,
    name: String,
    pipeline: drawing::Pipelines,
    store: crate::d3d12::residency::Store,
    fault: Option<Fault>,
    marker: PhantomData<Rc<()>>,
    setup: Setup,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Setup {
    pub driver: Duration,
    pub library: Duration,
    pub states: Duration,
}

#[derive(Clone)]
pub struct Work {
    completion: output::Completion,
    pub(crate) receipt: Arc<output::Receipt>,
}

impl D3d12 {
    pub fn notified(mut self, notify: shared::Notify) -> Self {
        self.notify = Some(notify);
        self
    }

    pub fn new() -> Result<Self, Fault> {
        Self::create(false)
    }

    pub fn warp() -> Result<Self, Fault> {
        Self::create(true)
    }

    fn create(warp: bool) -> Result<Self, Fault> {
        let start = std::time::Instant::now();
        let (device, name) = adapter::select(warp)?;
        let queue = createqueue(&device)?;
        let fence = createfence(&device)?;
        let frequency = queryfrequency(&queue);
        let driver = start.elapsed();
        let (pipeline, mut setup) = drawing::pipeline(&device)?;
        setup.driver = driver;
        Ok(Self {
            notify: None,
            device,
            queue,
            fence,
            value: 0,
            frequency,
            name,
            pipeline,
            store: crate::d3d12::residency::Store::new(),
            fault: None,
            marker: PhantomData,
            setup,
        })
    }

    pub fn inspect(&self, resource: crate::d3d12::Resource) -> crate::d3d12::Residency {
        self.store.report[resource as usize]
    }
    pub fn resources(&self, resource: crate::d3d12::Resource) -> shared::cache::Snapshot {
        self.store.snapshot(resource)
    }
    pub fn cache(
        &mut self,
        resource: crate::d3d12::Resource,
        command: shared::cache::Command,
    ) -> Result<(), Fault> {
        self.store.command(resource, command)
    }
    pub fn wait(&mut self, work: &Work, timeout: Duration) -> Result<(), Fault> {
        if work.receipt.queue != self.queue {
            return Err(Fault("foreign D3D12 work".into()));
        }
        work.completion.wait(timeout)?;
        match work.receipt.signal.state() {
            State::Complete => Ok(()),
            State::Failed(fault) => {
                self.fault = Some(fault.clone());
                Err(fault)
            }
            State::Pending => Err(Fault("D3D12 signalled incomplete work".into())),
        }
    }
    pub fn batching(&mut self, limit: usize) -> Result<(), Fault> {
        if !(1..=128).contains(&limit) {
            return Err(Fault("path batch limit requires 1 through 128".into()));
        }
        self.store.limit = limit;
        Ok(())
    }
    pub fn batches(&self) -> crate::d3d12::Batches {
        self.store.batches
    }
    pub fn pool(&mut self) -> &mut crate::d3d12::pool::Pool {
        &mut self.store.pool
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn setup(&self) -> Setup {
        self.setup
    }

    fn encode(&mut self, pass: &Pass) -> Result<Work, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        self.store.maintain();
        let allocator = memory::Allocator(&self.device);
        let texture = allocator.texture(pass.extent())?;
        let (pool, list) = allocator.command()?;
        let mut frame = drawing::Frame::new(&self.device, list.clone())?;
        let stamps = createtimestamps(&self.device, &list, self.frequency);
        let encoded = drawing::encode(
            (&self.device, &self.pipeline),
            pass,
            (&texture.resource, &mut frame),
            &mut self.store,
        );
        self.store.publish();
        if let Err(fault) = encoded {
            self.store.pool.discard();
            return Err(fault);
        }
        let stampres = finishtimestamps(&list, stamps);
        let mut retained = frame.retained;
        frame.arena.finish(&mut retained);
        retained.push(frame.rtvheap);
        retained.push(frame.srvheap);
        retained.push(pool);
        retained.push(list.clone());
        if let Some((heap, _)) = &stampres {
            retained.push(heap.clone());
        }
        let signal = output::commit(
            (&self.device, &self.queue),
            &list,
            (&self.fence, &mut self.value),
        )?;
        self.store.pool.submit(signal.clone());
        let completion = output::Completion::attach(signal.clone(), self.notify.clone());
        let receipt = Arc::new(output::Receipt {
            signal,
            queue: self.queue.clone(),
            texture: texture.resource,
            extent: pass.extent(),
            stamps: stampres.map(|(_, buffer)| (buffer, self.frequency)),
            retained,
        });
        Ok(Work {
            completion,
            receipt,
        })
    }
}

impl Backend for D3d12 {
    type Work = Work;

    fn submit(&mut self, pass: &Pass) -> Result<Work, Fault> {
        self.encode(pass)
    }

    fn poll(&mut self, work: &mut Work) -> Result<State, Fault> {
        let state = work.receipt.signal.state();
        if let State::Failed(fault) = &state {
            self.fault = Some(fault.clone());
            return Err(fault.clone());
        }
        Ok(state)
    }

    fn read(&mut self, work: &Work) -> Result<Image, Fault> {
        if work.receipt.queue != self.queue {
            return Err(Fault("foreign D3D12 work".into()));
        }
        output::read(
            (&self.device, &self.queue),
            (&self.fence, &mut self.value),
            &work.receipt,
        )
    }
}

impl Work {
    pub fn duration(&self) -> Option<Duration> {
        output::elapsed(&self.receipt)
    }
}

fn createqueue(device: &Com) -> Result<Com, Fault> {
    let desc = types::Queue::default();
    let mut raw = std::ptr::null_mut();
    // SAFETY: device is a live ID3D12Device creating a direct command queue.
    let hr = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.queue)(device.raw(), &desc, &constants::QUEUE, &mut raw)
    };
    check(hr, "ID3D12Device::CreateCommandQueue")?;
    // SAFETY: CreateCommandQueue succeeded and transferred ownership of raw.
    unsafe { Com::own(raw) }
}

pub(crate) fn createfence(device: &Com) -> Result<Com, Fault> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: device is a live ID3D12Device creating a fence initialized to zero.
    let hr = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.fence)(device.raw(), 0, 0, &constants::FENCE, &mut raw)
    };
    check(hr, "ID3D12Device::CreateFence")?;
    // SAFETY: CreateFence succeeded and transferred ownership of raw.
    unsafe { Com::own(raw) }
}

fn queryfrequency(queue: &Com) -> u64 {
    let mut freq = 0u64;
    // SAFETY: queue is a live direct ID3D12CommandQueue and freq is writable.
    let hr = unsafe {
        let table = queue.vtable::<tables::Queue>();
        (table.frequency)(queue.raw(), &mut freq)
    };
    if hr >= 0 { freq } else { 0 }
}

fn createtimestamps(device: &Com, list: &Com, frequency: u64) -> Option<(Com, Com)> {
    if frequency == 0 {
        return None;
    }
    let desc = types::Query {
        kind: constants::TIMEHEAP,
        count: 2,
        mask: 0,
    };
    let mut raw = std::ptr::null_mut();
    // SAFETY: device is live and desc defines a 2-slot timestamp query heap.
    let hr = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.query)(device.raw(), &desc, &constants::QUERY, &mut raw)
    };
    if hr < 0 {
        return None;
    }
    // SAFETY: CreateQueryHeap succeeded and transferred ownership.
    let heap = unsafe { Com::own(raw).ok()? };
    let buffer = memory::Allocator(device)
        .buffer(16, (constants::READBACK, constants::COPYDEST))
        .ok()?;
    // SAFETY: list is open and heap owns query slot 0.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.endquery)(list.raw(), heap.raw(), constants::TIMESTAMP, 0);
    }
    Some((heap, buffer))
}

fn finishtimestamps(list: &Com, stamps: Option<(Com, Com)>) -> Option<(Com, Com)> {
    let (heap, buffer) = stamps?;
    // SAFETY: list is open; heap owns slots 0..2 and buffer is a 16-byte COPY_DEST READBACK buffer.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.endquery)(list.raw(), heap.raw(), constants::TIMESTAMP, 1);
        (table.resolve)(
            list.raw(),
            heap.raw(),
            constants::TIMESTAMP,
            0,
            2,
            buffer.raw(),
            0,
        );
    }
    Some((heap, buffer))
}
