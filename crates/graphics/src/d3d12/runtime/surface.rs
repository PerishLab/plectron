use crate::d3d12::ffi::{Com, Handle};
use crate::d3d12::runtime::presentation::Worker;
use crate::d3d12::{Execution, Work};
use crate::shared::{Extent, Fault, Output, Sink, State, Target, Visibility};
use std::marker::PhantomData;
use std::rc::Rc;

pub struct Surface {
    notify: Option<shared::Notify>,
    handle: Handle,
    target: Target,
    queue: Option<Com>,
    extent: Extent,
    worker: Option<Worker>,
    marker: PhantomData<Rc<()>>,
}

impl Surface {
    pub fn notified(mut self, notify: shared::Notify) -> Self {
        self.notify = Some(notify);
        self
    }

    /// # Safety
    /// `handle` must be a live Win32 `HWND` owned by the current thread that outlives this `Surface`.
    pub unsafe fn new(target: Target, output: Output, handle: Handle) -> Result<Self, Fault> {
        if handle.is_null() {
            return Err(Fault("D3D12 surface requires live window handle".into()));
        }
        let extent = output.pixels();
        extent.area()?;
        Ok(Self {
            notify: None,
            handle,
            target,
            queue: None,
            extent,
            worker: None,
            marker: PhantomData,
        })
    }

    pub fn resize(&mut self, output: Output) -> Result<(), Fault> {
        let extent = output.pixels();
        extent.area()?;
        if extent == self.extent {
            return Ok(());
        }
        if let Some(worker) = self.worker.as_mut() {
            worker.resize(extent)?;
        }
        self.extent = extent;
        Ok(())
    }

    fn draw(&mut self, work: &Work) -> Result<(), Fault> {
        if work.receipt.extent != self.extent
            || self
                .queue
                .as_ref()
                .is_some_and(|queue| *queue != work.receipt.queue)
        {
            return Err(Fault("foreign or mismatched surface output".into()));
        }
        if work.receipt.signal.state() != State::Complete {
            return Err(Fault("presentation requires completed rendering".into()));
        }
        if self.worker.is_none() {
            self.queue = Some(work.receipt.queue.clone());
            self.worker = Some(Worker::new(
                self.handle,
                work,
                self.extent,
                self.notify.clone(),
            )?);
        }
        self.worker
            .as_mut()
            .expect("configured worker")
            .present(work)
    }
}

impl Sink<Work> for Surface {
    fn target(&self) -> Target {
        self.target
    }

    fn present(&mut self, work: &Work) -> Result<(), Fault> {
        self.draw(work)
    }

    fn completed(&self) -> Option<std::time::Instant> {
        self.worker.as_ref().and_then(Worker::completed)
    }

    fn visibility(&self) -> Visibility {
        self.worker
            .as_ref()
            .map_or(Visibility::Unobserved, Worker::visibility)
    }

    fn cancel(&mut self) {
        self.worker = None;
    }

    fn poll(&mut self) -> Result<State, Fault> {
        self.worker
            .as_mut()
            .ok_or_else(|| Fault("presentation is absent".into()))?
            .poll()
    }
}

impl Sink<Execution> for Surface {
    fn target(&self) -> Target {
        self.target
    }

    fn present(&mut self, work: &Execution) -> Result<(), Fault> {
        self.draw(work.artifact()?)
    }

    fn completed(&self) -> Option<std::time::Instant> {
        self.worker.as_ref().and_then(Worker::completed)
    }

    fn visibility(&self) -> Visibility {
        self.worker
            .as_ref()
            .map_or(Visibility::Unobserved, Worker::visibility)
    }

    fn cancel(&mut self) {
        self.worker = None;
    }

    fn poll(&mut self) -> Result<State, Fault> {
        self.worker
            .as_mut()
            .ok_or_else(|| Fault("presentation is absent".into()))?
            .poll()
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.worker = None;
    }
}
