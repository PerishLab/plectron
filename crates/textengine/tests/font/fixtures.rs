pub fn font(glyphs: &[Vec<u8>], cmap: Vec<u8>) -> Vec<u8> {
    let mut head = vec![0; 54];
    head[12..16].copy_from_slice(&0x5f0f3cf5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    head[50..52].copy_from_slice(&1u16.to_be_bytes());
    let mut hhea = vec![0; 36];
    hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
    hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
    hhea[34..36].copy_from_slice(&1u16.to_be_bytes());
    let mut maxp = vec![0; 6];
    maxp[4..6].copy_from_slice(&(glyphs.len() as u16).to_be_bytes());
    let mut hmtx = vec![0; 4 + (glyphs.len() - 1) * 2];
    hmtx[0..2].copy_from_slice(&500u16.to_be_bytes());
    let mut loca = Vec::new();
    let mut glyf = Vec::new();
    for glyph in glyphs {
        loca.extend((glyf.len() as u32).to_be_bytes());
        glyf.extend(glyph);
    }
    loca.extend((glyf.len() as u32).to_be_bytes());
    let tables = [
        (b"head", head),
        (b"hhea", hhea),
        (b"maxp", maxp),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"glyf", glyf),
        (b"cmap", cmap),
    ];
    let mut bytes = vec![0; 12 + tables.len() * 16];
    bytes[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    bytes[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    for (index, (tag, data)) in tables.into_iter().enumerate() {
        let offset = 12 + index * 16;
        let start = bytes.len() as u32;
        bytes[offset..offset + 4].copy_from_slice(tag);
        bytes[offset + 8..offset + 12].copy_from_slice(&start.to_be_bytes());
        bytes[offset + 12..offset + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend(data);
    }
    bytes
}
pub fn cmap() -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12];
    for word in [
        4u16, 32, 0, 4, 0, 0, 0, 65, 65535, 0, 65, 65535, 65472, 1, 0, 0,
    ] {
        bytes.extend(word.to_be_bytes());
    }
    bytes
}
pub fn simple(points: &[(i16, i16, bool)]) -> Vec<u8> {
    let mut bytes = vec![0; 10];
    bytes[1] = 1;
    bytes.extend(((points.len() - 1) as u16).to_be_bytes());
    bytes.extend([0, 0]);
    bytes.extend(points.iter().map(|point| u8::from(point.2)));
    for axis in 0..2 {
        let mut previous = 0;
        for &(x, y, _) in points {
            let value = if axis == 0 { x } else { y };
            bytes.extend((value - previous).to_be_bytes());
            previous = value;
        }
    }
    bytes
}
pub fn compound(parts: &[(u16, u16, [u16; 2])]) -> Vec<u8> {
    let mut bytes = vec![0; 10];
    bytes[0..2].copy_from_slice(&(-1i16).to_be_bytes());
    for &(flags, glyph, args) in parts {
        for word in [flags, glyph, args[0], args[1]] {
            bytes.extend(word.to_be_bytes());
        }
    }
    bytes
}
