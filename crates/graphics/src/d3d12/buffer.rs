use crate::d3d12::ffi::{Com, check, constants, tables, types};
use crate::shared::{Fault, Geometry, Rect};

pub(crate) type Entry<'a> = (&'a Geometry, u32);

#[derive(Clone)]
pub(crate) struct Path {
    pub resource: Com,
    pub address: u64,
    pub bytes: usize,
    pub allocated: usize,
}

#[derive(Default)]
pub(crate) struct Batch<'a> {
    pub entries: Vec<Entry<'a>>,
    bytes: usize,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Batches {
    pub paths: u64,
    pub calls: u64,
    pub overlaps: u64,
    pub limited: u64,
    pub switched: u64,
}

pub(crate) enum Split {
    Evaluation,
    Overlap,
    Limit,
}

impl<'a> Batch<'a> {
    pub fn boundary(&self, coverage: &Geometry, limit: usize) -> Option<Split> {
        if self.entries.is_empty() {
            return None;
        }
        if self.entries[0].0.evaluation() != coverage.evaluation() {
            return Some(Split::Evaluation);
        }
        if self.entries.len() >= limit || self.bytes + size(coverage) > 4_194_304 {
            return Some(Split::Limit);
        }
        self.entries
            .iter()
            .any(|(previous, _)| {
                let intersection = previous.bounds.intersect(coverage.bounds);
                intersection.width != 0 && intersection.height != 0
            })
            .then_some(Split::Overlap)
    }

    pub fn push(&mut self, entry: Entry<'a>) {
        self.bytes += size(entry.0);
        self.entries.push(entry);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

fn size(coverage: &Geometry) -> usize {
    std::mem::size_of_val(coverage.edges())
        + coverage.curves().len() * 32
        + std::mem::size_of_val(coverage.bands().data())
        + 44
}

pub(crate) struct Arena {
    device: Com,
    resource: Option<Com>,
    mapped: *mut u8,
    base: u64,
    capacity: usize,
    used: usize,
    retired: Vec<Com>,
}

impl Arena {
    pub fn new(device: Com) -> Self {
        Self {
            device,
            resource: None,
            mapped: std::ptr::null_mut(),
            base: 0,
            capacity: 0,
            used: 0,
            retired: Vec::new(),
        }
    }

    pub fn write(&mut self, bytes: &[u8], align: usize) -> Result<(Com, u64, u64), Fault> {
        let len = bytes.len().max(4);
        let start = self.used.next_multiple_of(align.max(4));
        if self.resource.is_none() || start.saturating_add(len) > self.capacity {
            self.grow(len.max(262_144))?;
        }
        let offset = self.used.next_multiple_of(align.max(4));
        // SAFETY: mapped is live for self.capacity bytes and offset + bytes.len() <= self.capacity.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.mapped.add(offset), bytes.len());
        }
        self.used = offset + len;
        let resource = self
            .resource
            .as_ref()
            .expect("allocated upload arena")
            .clone();
        Ok((resource, offset as u64, self.base + offset as u64))
    }

    fn grow(&mut self, minimum: usize) -> Result<(), Fault> {
        self.unmap();
        let capacity = minimum.next_power_of_two().max(262_144);
        let resource = crate::d3d12::memory::Allocator(&self.device)
            .buffer(capacity, (constants::UPLOAD, constants::GENERIC))?;
        let mut ptr = std::ptr::null_mut();
        let empty = types::Range { begin: 0, end: 0 };
        // SAFETY: resource is a newly created UPLOAD buffer; empty read range denotes write-only CPU access.
        let hr = unsafe {
            let table = resource.vtable::<tables::Resource>();
            (table.map)(resource.raw(), 0, &empty, &mut ptr)
        };
        check(hr, "ID3D12Resource::Map")?;
        // SAFETY: resource is a live buffer resource with a valid GPU virtual address.
        let base = unsafe {
            let table = resource.vtable::<tables::Resource>();
            (table.address)(resource.raw())
        };
        self.resource = Some(resource);
        self.mapped = ptr.cast::<u8>();
        self.base = base;
        self.capacity = capacity;
        self.used = 0;
        Ok(())
    }

    fn unmap(&mut self) {
        if let Some(previous) = self.resource.take() {
            // SAFETY: previous was mapped by grow(); passing null unmaps the entire written range.
            unsafe {
                let table = previous.vtable::<tables::Resource>();
                (table.unmap)(previous.raw(), 0, std::ptr::null());
            }
            self.retired.push(previous);
        }
        self.mapped = std::ptr::null_mut();
        self.base = 0;
        self.capacity = 0;
        self.used = 0;
    }

    pub fn finish(mut self, target: &mut Vec<Com>) {
        self.unmap();
        target.append(&mut self.retired);
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        self.unmap();
    }
}

pub(crate) fn stage(
    device: &Com,
    list: &Com,
    arena: &mut Arena,
    data: (&[u8], u32),
) -> Result<(Com, u64, usize), Fault> {
    let (bytes, state) = data;
    let size = bytes.len().max(4);
    let (resource, allocated) = crate::d3d12::memory::Allocator(device)
        .bufferinfo(size, (constants::DEFAULT, constants::COPYDEST))?;
    let (upload, offset, _) = arena.write(bytes, 16)?;
    // SAFETY: upload is in GENERIC_READ, resource is in COPY_DEST, and size fits both buffers.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.copybuffer)(
            list.raw(),
            resource.raw(),
            0,
            upload.raw(),
            offset,
            size as u64,
        );
    }
    transition(list, &resource, constants::COPYDEST, state);
    // SAFETY: resource is a live DEFAULT buffer with a valid GPU virtual address.
    let address = unsafe {
        let table = resource.vtable::<tables::Resource>();
        (table.address)(resource.raw())
    };
    Ok((resource, address, allocated))
}

pub(crate) fn transition(list: &Com, resource: &Com, before: u32, after: u32) {
    if before == after {
        return;
    }
    let barrier = types::Barrier {
        kind: 0,
        flags: 0,
        transition: types::Transition {
            resource: resource.raw(),
            subresource: constants::ALLSUB,
            before,
            after,
        },
    };
    // SAFETY: list is open and resource is a live ID3D12Resource transitioning from before to after.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.barrier)(list.raw(), 1, &barrier);
    }
}

pub(crate) fn path(
    device: &Com,
    list: &Com,
    arena: &mut Arena,
    entries: &[Entry<'_>],
) -> Result<Path, Fault> {
    let mut words = vec![0u32; entries.len() * 8];
    for (index, (coverage, _)) in entries.iter().enumerate() {
        words.resize(words.len().next_multiple_of(4), 0);
        let edge = words.len();
        words.extend(
            coverage
                .edges()
                .iter()
                .flatten()
                .map(|value| value.to_bits()),
        );
        words.extend(
            coverage
                .curves()
                .iter()
                .flat_map(|curve| packed(curve).map(f32::to_bits)),
        );
        let band = words.len();
        words.extend_from_slice(coverage.bands().data());
        words[index * 8..index * 8 + 4].copy_from_slice(&[
            edge as u32,
            band as u32,
            coverage.bands().origin() as u32,
            coverage.bands().stride() as u32,
        ]);
        words[index * 8 + 4..index * 8 + 8]
            .copy_from_slice(&coverage.clip().edges().map(f32::to_bits));
    }
    let length = std::mem::size_of_val(words.as_slice());
    // SAFETY: words is a contiguous initialized [u32] slice reinterpreted as read-only bytes.
    let raw = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), length) };
    let (resource, address, allocated) = stage(device, list, arena, (raw, constants::PIXEL))?;
    Ok(Path {
        resource,
        address,
        bytes: length,
        allocated,
    })
}

pub(crate) fn draw(
    list: &Com,
    arena: &mut Arena,
    path: &Path,
    region: (Rect, &[Entry<'_>]),
) -> Result<(), Fault> {
    let (extent, entries) = region;
    let placements = crate::d3d12::vertices::placements(extent, entries);
    let length = std::mem::size_of_val(placements.as_slice());
    // SAFETY: placements is a contiguous initialized [f32] slice reinterpreted as read-only bytes.
    let bytes = unsafe { std::slice::from_raw_parts(placements.as_ptr().cast::<u8>(), length) };
    let (_, _, address) = arena.write(bytes, 16)?;
    // SAFETY: Root param 1 binds vertex placements (t0), root param 2 binds path data (t1), and each instance addresses its own header.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.srv)(list.raw(), 1, address);
        (table.srv)(list.raw(), 2, path.address);
        (table.draw)(list.raw(), 6, entries.len() as u32, 0, 0);
    }
    Ok(())
}

fn packed(curve: &crate::shared::Curve) -> [f32; 8] {
    let [p, q, r] = curve.points;
    let dir = curve.direction as f32;
    let quad = u8::from(curve.quadratic) as f32;
    [p[0], p[1], q[0], q[1], r[0], r[1], dir, quad]
}
