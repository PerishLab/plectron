use super::*;
use crate::shared::Extent;
use x11rb::image;
use x11rb::protocol;

fn display() -> bool {
    let available = Connection::open().is_ok();
    if !available {
        eprintln!("skip linux::window::pixels: no reachable X11 display");
    }
    available
}

fn drain(window: &mut Window, connection: &Connection) {
    for _ in 0..200 {
        while let Some(event) = connection.take().unwrap() {
            if let protocol::Event::PresentCompleteNotify(notify) = &event {
                window.sight(notify);
            } else {
                window.dispatch(&event).unwrap();
            }
        }
        if !window.sighted().is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
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
            if matches!(event, protocol::Event::MapNotify(_)) {
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
    let serial = window
        .install(Image {
            extent: Extent {
                width: 64,
                height: 32,
            },
            pixels,
        })
        .unwrap()
        .expect("present serial");
    window.sightings.watch(serial, 1);
    drain(&mut window, &connection);
    connection.conn.get_input_focus().unwrap().reply().unwrap();
    let (capture, _) = image::Image::get(&connection.conn, window.xid, 0, 0, 64, 32).unwrap();
    assert_eq!(
        (capture.get_pixel(16, 16), capture.get_pixel(48, 16)),
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
