use crate::d3d12::ffi::{Com, check, constants, loader, pipeline as raw, tables, types};
use crate::d3d12::resource;
use crate::shared::Fault;
use std::sync::OnceLock;

pub struct Pipelines {
    pub(super) root: Com,
    pub(super) coverage: Com,
    pub(super) solid: Com,
    pub(super) composite: Com,
    pub(super) curve: Com,
    pub(super) path: Com,
    pub(super) image: Com,
}

struct Compiled {
    vmain: Vec<u8>,
    vpath: Vec<u8>,
    vcoverage: Vec<u8>,
    fmain: Vec<u8>,
    fcomposite: Vec<u8>,
    fpath: Vec<u8>,
    fcurve: Vec<u8>,
    fimage: Vec<u8>,
    fcoverage: Vec<u8>,
}

static SHADERS: OnceLock<Result<Compiled, String>> = OnceLock::new();

pub fn pipeline(device: &Com) -> Result<(Pipelines, crate::d3d12::Setup), Fault> {
    let start = std::time::Instant::now();
    let shaders = SHADERS
        .get_or_init(|| compileall().map_err(|fault| fault.0))
        .as_ref()
        .map_err(|message| Fault(message.clone()))?;
    let library = start.elapsed();
    let start = std::time::Instant::now();
    let root = signature(device)?;
    let pipelines = Pipelines {
        coverage: create(
            device,
            &root,
            (&shaders.vcoverage, &shaders.fcoverage),
            true,
        )?,
        solid: create(device, &root, (&shaders.vmain, &shaders.fmain), false)?,
        composite: create(device, &root, (&shaders.vmain, &shaders.fcomposite), true)?,
        curve: create(device, &root, (&shaders.vpath, &shaders.fcurve), true)?,
        path: create(device, &root, (&shaders.vpath, &shaders.fpath), true)?,
        image: create(device, &root, (&shaders.vmain, &shaders.fimage), true)?,
        root,
    };
    Ok((
        pipelines,
        crate::d3d12::Setup {
            driver: Default::default(),
            library,
            states: start.elapsed(),
        },
    ))
}

fn compileall() -> Result<Compiled, Fault> {
    let source = resource::shader()?.as_bytes();
    Ok(Compiled {
        vmain: compile(source, c"vertexMain", c"vs_5_0")?,
        vpath: compile(source, c"vertexPath", c"vs_5_0")?,
        vcoverage: compile(source, c"vertexCoverage", c"vs_5_0")?,
        fmain: compile(source, c"fragmentMain", c"ps_5_0")?,
        fcomposite: compile(source, c"fragmentComposite", c"ps_5_0")?,
        fpath: compile(source, c"fragmentPath", c"ps_5_0")?,
        fcurve: compile(source, c"fragmentCurve", c"ps_5_0")?,
        fimage: compile(source, c"fragmentImage", c"ps_5_0")?,
        fcoverage: compile(source, c"fragmentCoverage", c"ps_5_0")?,
    })
}

fn compile(
    source: &[u8],
    entry: &std::ffi::CStr,
    target: &std::ffi::CStr,
) -> Result<Vec<u8>, Fault> {
    let api = loader()?;
    let mut code = std::ptr::null_mut();
    let mut errors = std::ptr::null_mut();
    // SAFETY: source is live UTF-8 HLSL bytes; entry and target are NUL-terminated C strings.
    let hr = unsafe {
        (api.compile)(
            source.as_ptr().cast(),
            source.len(),
            c"draw.hlsl".as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            entry.as_ptr(),
            target.as_ptr(),
            constants::OPTIMIZE,
            0,
            &mut code,
            &mut errors,
        )
    };
    if hr < 0 {
        let message = diagnostic(errors);
        return Err(Fault(format!("HLSL compile failed: {message}")));
    }
    if !errors.is_null() {
        // SAFETY: errors is an owned ID3D10Blob returned alongside warnings.
        let _ = unsafe { Com::own(errors) };
    }
    // SAFETY: D3DCompile succeeded and returned an owned ID3D10Blob in code.
    let blob = unsafe { Com::own(code)? };
    Ok(bytes(&blob))
}

fn diagnostic(raw: *mut std::ffi::c_void) -> String {
    if raw.is_null() {
        return "unknown compiler error".into();
    }
    // SAFETY: raw is an owned ID3D10Blob returned by D3DCompile or D3D12SerializeRootSignature.
    let Ok(blob) = (unsafe { Com::own(raw) }) else {
        return "unknown compiler error".into();
    };
    String::from_utf8_lossy(&bytes(&blob)).trim().to_owned()
}

fn bytes(blob: &Com) -> Vec<u8> {
    // SAFETY: blob is a live ID3D10Blob whose buffer pointer and size stay valid while retained.
    unsafe {
        let table = blob.vtable::<tables::Blob>();
        let ptr = (table.pointer)(blob.raw()).cast::<u8>();
        let len = (table.size)(blob.raw());
        if ptr.is_null() || len == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(ptr, len).to_vec()
        }
    }
}

fn signature(device: &Com) -> Result<Com, Fault> {
    let api = loader()?;
    let range = raw::Segment {
        kind: 0,
        count: 1,
        register: 2,
        space: 0,
        offset: 0,
    };
    let params = [
        raw::Parameter {
            kind: constants::CONSTANTS,
            data: raw::Payload {
                constants: raw::Constants {
                    register: 0,
                    space: 0,
                    count: 10,
                },
            },
            visibility: constants::VISALL,
        },
        raw::Parameter {
            kind: constants::SRV,
            data: raw::Payload {
                descriptor: raw::Descriptor {
                    register: 0,
                    space: 0,
                },
            },
            visibility: constants::VISVERTEX,
        },
        raw::Parameter {
            kind: constants::SRV,
            data: raw::Payload {
                descriptor: raw::Descriptor {
                    register: 1,
                    space: 0,
                },
            },
            visibility: constants::VISPIXEL,
        },
        raw::Parameter {
            kind: constants::TABLE,
            data: raw::Payload {
                table: raw::Table {
                    count: 1,
                    ranges: &range,
                },
            },
            visibility: constants::VISPIXEL,
        },
    ];
    let desc = raw::Signature {
        count: params.len() as u32,
        params: params.as_ptr(),
        samplers: 0,
        statics: std::ptr::null(),
        flags: 0,
    };
    let mut blob = std::ptr::null_mut();
    let mut errors = std::ptr::null_mut();
    // SAFETY: desc references stack-pinned params and range for the duration of serialization.
    let hr = unsafe { (api.serialize)(&desc, 1, &mut blob, &mut errors) };
    if hr < 0 {
        let message = diagnostic(errors);
        return Err(Fault(format!(
            "root signature serialization failed: {message}"
        )));
    }
    if !errors.is_null() {
        // SAFETY: errors is an owned warning blob returned by D3D12SerializeRootSignature.
        let _ = unsafe { Com::own(errors) };
    }
    // SAFETY: D3D12SerializeRootSignature returned an owned blob on success.
    let blob = unsafe { Com::own(blob)? };
    let data = bytes(&blob);
    let mut out = std::ptr::null_mut();
    // SAFETY: device is a live ID3D12Device and data holds a serialized root signature blob.
    let hr = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.root)(
            device.raw(),
            0,
            data.as_ptr().cast(),
            data.len(),
            &constants::ROOT,
            &mut out,
        )
    };
    check(hr, "ID3D12Device::CreateRootSignature")?;
    // SAFETY: CreateRootSignature succeeded and wrote an owned ID3D12RootSignature into out.
    unsafe { Com::own(out) }
}

fn create(device: &Com, root: &Com, stages: (&[u8], &[u8]), blend: bool) -> Result<Com, Fault> {
    let mut blending = raw::Blend::default();
    blending.targets[0] = raw::Attachment {
        enable: i32::from(blend),
        logic: 0,
        source: constants::ONE,
        dest: constants::INVSRCALPHA,
        op: constants::ADD,
        srcalpha: constants::ONE,
        dstalpha: constants::INVSRCALPHA,
        opalpha: constants::ADD,
        logicop: 0,
        mask: 0x0f,
    };
    let mut formats = [0u32; 8];
    formats[0] = constants::BGRA;
    let desc = raw::Pipeline {
        root: root.raw(),
        vs: raw::Bytecode {
            ptr: stages.0.as_ptr().cast(),
            len: stages.0.len(),
        },
        ps: raw::Bytecode {
            ptr: stages.1.as_ptr().cast(),
            len: stages.1.len(),
        },
        ds: raw::Bytecode::default(),
        hs: raw::Bytecode::default(),
        gs: raw::Bytecode::default(),
        stream: raw::Stream::default(),
        blend: blending,
        mask: u32::MAX,
        raster: raw::Raster {
            fill: constants::SOLID,
            cull: constants::CULLNONE,
            clip: 1,
            ..raw::Raster::default()
        },
        depth: raw::Depth::default(),
        input: raw::Layout::default(),
        cut: 0,
        topology: constants::TRIANGLE,
        targets: 1,
        formats,
        dsv: 0,
        sample: types::Sample {
            count: 1,
            quality: 0,
        },
        node: 0,
        cached: raw::Cache::default(),
        flags: 0,
    };
    let mut out = std::ptr::null_mut();
    // SAFETY: device and root are live; stages slices remain valid throughout CreateGraphicsPipelineState.
    let hr = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.pipeline)(device.raw(), &desc, &constants::PIPELINE, &mut out)
    };
    check(hr, "ID3D12Device::CreateGraphicsPipelineState")?;
    // SAFETY: CreateGraphicsPipelineState succeeded and wrote an owned PSO into out.
    unsafe { Com::own(out) }
}
