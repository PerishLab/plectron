use super::{Com, check, constants, loader, tables, types};
use crate::shared::Fault;

pub(crate) fn select(warp: bool) -> Result<(Com, String), Fault> {
    let api = loader()?;
    let mut raw = std::ptr::null_mut();
    // SAFETY: CreateDXGIFactory2 populates raw with an owned IDXGIFactory4 on success.
    let hr = unsafe { (api.factory)(0, &constants::FACTORY4, &mut raw) };
    check(hr, "CreateDXGIFactory2")?;
    // SAFETY: CreateDXGIFactory2 succeeded and returned an owned IDXGIFactory4.
    let factory = unsafe { Com::own(raw)? };
    if !warp && let Some(pair) = hardware(&factory) {
        return Ok(pair);
    }
    software(&factory)
}

fn hardware(factory: &Com) -> Option<(Com, String)> {
    if let Some(factory6) = factory.query(&constants::FACTORY6) {
        let mut index = 0u32;
        loop {
            let mut raw = std::ptr::null_mut();
            // SAFETY: EnumAdapterByGpuPreference writes an owned IDXGIAdapter1 on success.
            let hr = unsafe {
                let table = factory6.vtable::<tables::Factory6>();
                (table.prefer)(
                    factory6.raw(),
                    index,
                    constants::PERFORMANCE,
                    &constants::ADAPTER,
                    &mut raw,
                )
            };
            if hr < 0 {
                break;
            }
            index += 1;
            // SAFETY: EnumAdapterByGpuPreference succeeded and transferred adapter ownership.
            if let Ok(adapter) = unsafe { Com::own(raw) }
                && let Some(pair) = probe(&adapter, true)
            {
                return Some(pair);
            }
        }
    }
    let mut index = 0u32;
    loop {
        let mut raw = std::ptr::null_mut();
        // SAFETY: EnumAdapters1 writes an owned IDXGIAdapter1 on success.
        let hr = unsafe {
            let table = factory.vtable::<tables::Factory4>();
            (table.adapters)(factory.raw(), index, &mut raw)
        };
        if hr < 0 {
            break;
        }
        index += 1;
        // SAFETY: EnumAdapters1 succeeded and transferred adapter ownership.
        if let Ok(adapter) = unsafe { Com::own(raw) }
            && let Some(pair) = probe(&adapter, true)
        {
            return Some(pair);
        }
    }
    None
}

fn software(factory: &Com) -> Result<(Com, String), Fault> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: EnumWarpAdapter writes an owned WARP IDXGIAdapter1 on success.
    let hr = unsafe {
        let table = factory.vtable::<tables::Factory4>();
        (table.warp)(factory.raw(), &constants::ADAPTER, &mut raw)
    };
    check(hr, "IDXGIFactory4::EnumWarpAdapter")?;
    // SAFETY: EnumWarpAdapter succeeded and transferred adapter ownership.
    let adapter = unsafe { Com::own(raw)? };
    probe(&adapter, false).ok_or_else(|| Fault("D3D12 device unavailable".into()))
}

fn probe(adapter: &Com, skip: bool) -> Option<(Com, String)> {
    let mut desc = types::Adapter::default();
    // SAFETY: adapter is a live IDXGIAdapter1 and desc is writable.
    let hr = unsafe {
        let table = adapter.vtable::<tables::Adapter>();
        (table.desc)(adapter.raw(), &mut desc)
    };
    if hr < 0 || (skip && (desc.flags & constants::SOFTWARE) != 0) {
        return None;
    }
    let api = loader().ok()?;
    let mut raw = std::ptr::null_mut();
    // SAFETY: D3D12CreateDevice writes an owned ID3D12Device into raw on success.
    let hr = unsafe {
        (api.device)(
            adapter.raw(),
            constants::LEVEL,
            &constants::DEVICE,
            &mut raw,
        )
    };
    if hr < 0 {
        return None;
    }
    // SAFETY: D3D12CreateDevice succeeded and transferred ownership of raw.
    let device = unsafe { Com::own(raw).ok()? };
    let end = desc
        .name
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(desc.name.len());
    Some((device, String::from_utf16_lossy(&desc.name[..end])))
}
