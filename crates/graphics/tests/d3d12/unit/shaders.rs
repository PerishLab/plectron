use crate::d3d12::{D3d12, ffi, resource};
use crate::shared::{Backend, Cpu, Extent, Font, Frame, Glyph, Image, Item, Rect, Scene, Visual};
use std::time::Duration;

#[test]
fn table() {
    let shaders = resource::shaders().unwrap();
    let blobs = [
        shaders.vmain,
        shaders.vpath,
        shaders.vcoverage,
        shaders.fmain,
        shaders.fcomposite,
        shaders.fpath,
        shaders.fcurve,
        shaders.fimage,
        shaders.fcoverage,
    ];
    for blob in blobs {
        assert!(blob.len() > 32);
        assert_eq!(&blob[..4], b"DXBC");
    }
    assert!(resource::decode(b"").is_err());
    assert!(resource::decode(b"XXXX\x01\0\0\0\x09\0\0\0").is_err());
    static BADVERSION: [u8; 84] = [
        b'P', b'D', b'X', b'B', 2, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(resource::decode(&BADVERSION).is_err());
    static BADCOUNT: [u8; 84] = [
        b'P', b'D', b'X', b'B', 1, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(resource::decode(&BADCOUNT).is_err());
    static BADBOUNDS: [u8; 84] = [
        b'P', b'D', b'X', b'B', 1, 0, 0, 0, 9, 0, 0, 0, 84, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(resource::decode(&BADBOUNDS).is_err());
}

#[test]
fn precompiled() {
    let mut d3d12 = D3d12::new().unwrap();
    assert!(
        d3d12.setup().library < Duration::from_millis(2),
        "setup.library took {:?}",
        d3d12.setup().library
    );
    assert!(
        !ffi::loaded("d3dcompiler_47.dll"),
        "d3dcompiler_47.dll must not be loaded on cold D3D12 startup"
    );
    let extent = Extent {
        width: 96,
        height: 48,
    };
    let font = Font {
        face: fixtures::face("lato", crate::shared::Weight::Normal),
        size: 28.into(),
        ..Font::default()
    };
    let scene = Scene::new(
        extent,
        1,
        vec![
            Item::Fill {
                rect: Rect {
                    x: 4,
                    y: 4,
                    width: 88,
                    height: 40,
                },
                color: Some(0x1f2937),
            },
            Item::Text {
                glyphs: vec![Glyph {
                    id: font.index('g'),
                    x: 12.into(),
                    y: 8.into(),
                }],
                font,
                color: 0xf8fafc,
                clip: None,
            },
        ],
    )
    .unwrap();
    let frame = Frame::new(scene, Visual::new(extent)).unwrap();
    let work = d3d12.render(&frame).unwrap();
    d3d12.wait(&work, Duration::from_secs(5)).unwrap();
    let actual = d3d12.read(&work).unwrap();
    let expected = Cpu.render(&frame).unwrap();
    assert!(difference(&actual, &expected) <= 1);
    let bitmap = Frame::new(Scene::bitmap(&expected).unwrap(), Visual::new(extent)).unwrap();
    let work = d3d12.render(&bitmap).unwrap();
    d3d12.wait(&work, Duration::from_secs(5)).unwrap();
    let actual = d3d12.read(&work).unwrap();
    assert!(difference(&actual, &expected) <= 1);
    assert!(!ffi::loaded("d3dcompiler_47.dll"));
}

fn difference(left: &Image, right: &Image) -> u32 {
    assert_eq!(left.extent, right.extent);
    left.pixels
        .iter()
        .zip(&right.pixels)
        .flat_map(|(a, b)| [0, 8, 16].map(|s| ((a >> s) & 255).abs_diff((b >> s) & 255)))
        .max()
        .unwrap_or(0)
}
