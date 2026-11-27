use crate::shared::Fault;

const MAGIC: &[u8; 4] = b"PDXB";
const VERSION: u32 = 1;
const COUNT: usize = 9;
const HEADER: usize = 12 + COUNT * 8;

static TABLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/draw.dxbc"));

#[derive(Clone, Copy)]
pub struct Shaders {
    pub vmain: &'static [u8],
    pub vpath: &'static [u8],
    pub vcoverage: &'static [u8],
    pub fmain: &'static [u8],
    pub fcomposite: &'static [u8],
    pub fpath: &'static [u8],
    pub fcurve: &'static [u8],
    pub fimage: &'static [u8],
    pub fcoverage: &'static [u8],
}

pub fn shaders() -> Result<Shaders, Fault> {
    decode(TABLE)
}

pub(crate) fn decode(bytes: &'static [u8]) -> Result<Shaders, Fault> {
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return Err(Fault("precompiled DXBC shader table is invalid".into()));
    }
    if word(bytes, 4) != VERSION || word(bytes, 8) as usize != COUNT {
        return Err(Fault(
            "precompiled DXBC shader table version mismatch".into(),
        ));
    }
    let mut entries = [&bytes[..0]; COUNT];
    for (index, slot) in entries.iter_mut().enumerate() {
        let base = 12 + index * 8;
        let offset = word(bytes, base) as usize;
        let len = word(bytes, base + 4) as usize;
        let end = offset
            .checked_add(len)
            .filter(|end| *end <= bytes.len() && offset >= HEADER && len > 0)
            .ok_or_else(|| Fault("precompiled DXBC shader entry out of bounds".into()))?;
        *slot = &bytes[offset..end];
    }
    Ok(Shaders {
        vmain: entries[0],
        vpath: entries[1],
        vcoverage: entries[2],
        fmain: entries[3],
        fcomposite: entries[4],
        fpath: entries[5],
        fcurve: entries[6],
        fimage: entries[7],
        fcoverage: entries[8],
    })
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
