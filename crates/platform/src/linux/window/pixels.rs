use super::*;
use crate::shared::Extent;
use x11rb::image::Image as XImage;
use x11rb::protocol::Event as XEvent;

fn display() -> bool {
    let available = Connection::open().is_ok();
    if !available {
        eprintln!("skip linux::window::pixels: no reachable X11 display");
    }
    available
}

#[test]
fn pixels() {
    if !display() {
        return;
    }
    let connection = Rc::new(Connection::open().unwrap());
    let mut window = Window::new(
        connection.clone(),
        &Options {
            title: "Plectron native bitmap probe".into(),
            extent: Extent {
                width: 64,
                height: 32,
            },
        },
    )
    .unwrap();
    window.show().unwrap();
    let mut mapped = false;
    for _ in 0..100 {
        while let Some(event) = connection.take().unwrap() {
            if matches!(event, XEvent::MapNotify(_)) {
                mapped = true;
            }
            window.dispatch(&event).unwrap();
        }
        if mapped {
            break;
        }
        connection.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(mapped, "window was not mapped");
    let geometry = connection
        .conn
        .get_geometry(window.xid)
        .unwrap()
        .reply()
        .unwrap();
    assert!(geometry.width >= 64 && geometry.height >= 32);
    let pixels = (0..32)
        .flat_map(|_| (0..64).map(|x| if x < 32 { 0xff0000 } else { 0x0000ff }))
        .collect();
    window
        .present(Image {
            extent: Extent {
                width: 64,
                height: 32,
            },
            pixels,
        })
        .unwrap();
    connection.conn.get_input_focus().unwrap().reply().unwrap();
    let (image, _) = XImage::get(&connection.conn, window.xid, 0, 0, 64, 32).unwrap();
    assert_eq!(
        (image.get_pixel(16, 16), image.get_pixel(48, 16)),
        (0xff0000, 0x0000ff)
    );
    assert!(
        window
            .present(Image {
                extent: Extent {
                    width: 1,
                    height: 1
                },
                pixels: vec![]
            })
            .is_err()
    );
    let xid = window.xid;
    window.closing = true;
    assert!(window.closing());
    assert!(!window.closing());
    drop(window);
    assert!(
        connection
            .conn
            .get_window_attributes(xid)
            .unwrap()
            .reply()
            .is_err()
    );
}
