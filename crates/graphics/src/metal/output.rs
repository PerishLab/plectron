use crate::metal::device::Work;
use crate::shared::{Fault, Image, State};
use objc2::runtime::ProtocolObject;
use objc2_metal::*;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

type Device = ProtocolObject<dyn MTLDevice>;
type Queue = ProtocolObject<dyn MTLCommandQueue>;
type Command = ProtocolObject<dyn MTLCommandBuffer>;

pub fn state(command: &Command) -> State {
    match command.status() {
        MTLCommandBufferStatus::Completed => State::Complete,
        MTLCommandBufferStatus::Error => State::Failed(Fault(
            command
                .error()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "Metal execution failed".into()),
        )),
        _ => State::Pending,
    }
}

#[derive(Clone)]
pub(crate) struct Completion(Arc<(Mutex<bool>, Condvar)>);
impl Completion {
    pub fn attach(command: &Command, notify: Option<shared::Notify>) -> Self {
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let sender = signal.clone();
        let callback = block2::RcBlock::new(move |_: std::ptr::NonNull<Command>| {
            *sender.0.lock().expect("completion lock") = true;
            sender.1.notify_all();
            if let Some(notify) = &notify {
                notify.notify();
            }
        });
        // SAFETY: Registered before commit; Metal copies the live block. It captures thread-safe completion state and an optional wake handle, never renderer or DOM state.
        unsafe {
            command.addCompletedHandler(block2::RcBlock::as_ptr(&callback));
        }
        Self(signal)
    }
    pub fn wait(&self, timeout: Duration) -> Result<(), Fault> {
        let ready = self.0.0.lock().expect("completion lock");
        let (ready, _) = self
            .0
            .1
            .wait_timeout_while(ready, timeout, |ready| !*ready)
            .expect("completion lock");
        if *ready {
            Ok(())
        } else {
            Err(Fault("Metal completion timed out".into()))
        }
    }
}
pub fn commit(command: &Command) -> Result<(), Fault> {
    let completion = Completion::attach(command, None);
    command.commit();
    completion.wait(Duration::from_secs(5))?;
    match state(command) {
        State::Complete => Ok(()),
        State::Failed(fault) => Err(fault),
        State::Pending => Err(Fault("Metal signalled incomplete work".into())),
    }
}

pub fn read(hardware: (&Device, &Queue), work: &Work) -> Result<Image, Fault> {
    if state(&work.command) != State::Complete {
        return Err(Fault("readback requires completed rendering".into()));
    }
    let stride = (work.extent.width * 4).div_ceil(256) * 256;
    let size = stride * work.extent.height;
    let buffer = hardware
        .0
        .newBufferWithLength_options(size, MTLResourceOptions::StorageModeShared)
        .ok_or_else(|| Fault("readback allocation failed".into()))?;
    let command = hardware
        .1
        .commandBuffer()
        .ok_or_else(|| Fault("readback command allocation failed".into()))?;
    let encoder = command
        .blitCommandEncoder()
        .ok_or_else(|| Fault("blit encoder allocation failed".into()))?;
    // SAFETY: Source rendering completed; the private texture is immutable afterwards.
    // The shared destination has stride * height bytes, and all regions are in bounds.
    // The command retains both resources; CPU access occurs only after completion.
    unsafe {
        encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
            &work.texture, 0, 0, MTLOrigin { x: 0, y: 0, z: 0 },
            MTLSize { width: work.extent.width, height: work.extent.height, depth: 1 },
            &buffer, 0, stride, size,
        );
    }
    encoder.endEncoding();
    commit(&command)?;
    // SAFETY: This unified-memory buffer is CPU-readable, retained locally, and its
    // only GPU writer has completed. The slice is bounded by the allocated size.
    let bytes =
        unsafe { std::slice::from_raw_parts(buffer.contents().as_ptr().cast::<u8>(), size) };
    let mut pixels = Vec::with_capacity(work.extent.area()?);
    for row in bytes.chunks(stride) {
        for pixel in row[..work.extent.width * 4].chunks_exact(4) {
            pixels.push(
                u32::from(pixel[0]) | (u32::from(pixel[1]) << 8) | (u32::from(pixel[2]) << 16),
            );
        }
    }
    Ok(Image {
        extent: work.extent,
        pixels,
    })
}
