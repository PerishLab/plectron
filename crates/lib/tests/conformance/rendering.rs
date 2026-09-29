use super::device::Device;
use plectron::contract::{Extent, Fault, Window};
use plectron::platform::{Host, Platform};
use plectron::rendering::{Image, Renderer, Solid};

#[test]
fn pixels() {
    let extent = Extent {
        width: 3,
        height: 2,
    };
    let frame = Solid.paint(extent, 0x123456).unwrap();
    assert_eq!(frame.extent, extent);
    assert_eq!(frame.pixels, [0x123456; 6]);
    assert!(Solid.paint(extent, 0x01000000).is_err());
    assert!(
        Solid
            .paint(
                Extent {
                    width: 0,
                    height: 2
                },
                0
            )
            .is_err()
    );
}

struct Oversized;

impl Renderer for Oversized {
    fn paint(&mut self, extent: Extent, color: u32) -> Result<Image, Fault> {
        Ok(Image {
            extent,
            pixels: vec![color; extent.area().map_err(|error| Fault(error.to_string()))? + 1],
        })
    }
}

#[test]
fn boundary() {
    let device = Device::default();
    let mut platform = Platform::new(device.clone(), Oversized);
    let unit = platform
        .window(Window {
            title: "test".into(),
            extent: Extent {
                width: 2,
                height: 2,
            },
        })
        .unwrap();
    let error = platform.fill(unit, 0).unwrap_err();
    assert!(error.0.contains("assigned extent"));
    assert_eq!(device.0.borrow().activated, [unit]);
    assert!(device.0.borrow().frames.is_empty());
}
