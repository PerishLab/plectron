use plectron_observe::snap::{Layout, Order, ppm};

fn layout(order: Order) -> Layout {
    Layout {
        width: 2,
        height: 2,
        depth: 24,
        bits: 32,
        pad: 32,
        order,
        masks: [0x00ff_0000, 0x0000_ff00, 0x0000_00ff],
    }
}

#[test]
fn converts_lsb_pixels() {
    let data = [
        0x33, 0x22, 0x11, 0, 0x66, 0x55, 0x44, 0, 0x99, 0x88, 0x77, 0, 0xcc, 0xbb, 0xaa, 0,
    ];
    let frame = ppm(&layout(Order::Lsb), &data).unwrap();
    assert_eq!(&frame[..11], b"P6\n2 2\n255\n");
    assert_eq!(
        &frame[11..],
        &[
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc
        ]
    );
}

#[test]
fn converts_msb_pixels() {
    let data = [0, 0x11, 0x22, 0x33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let frame = ppm(&layout(Order::Msb), &data).unwrap();
    assert_eq!(&frame[11..14], &[0x11, 0x22, 0x33]);
}

#[test]
fn refuses_other_visuals() {
    let mut value = layout(Order::Lsb);
    value.depth = 16;
    value.bits = 16;
    assert!(ppm(&value, &[0; 8]).is_err());
    let mut value = layout(Order::Lsb);
    value.masks = [0xff, 0xff00, 0xff_0000];
    assert!(ppm(&value, &[0; 16]).is_err());
    assert!(ppm(&layout(Order::Lsb), &[0; 15]).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn captures_known_fill() {
    use plectron_observe::snap::{Config, run};
    use x11rb::connection::Connection;
    use x11rb::protocol::Event;
    use x11rb::protocol::xproto::{ConnectionExt, CreateWindowAux, EventMask, WindowClass};

    let Ok((connection, screen)) = x11rb::connect(None) else {
        eprintln!("skip captures_known_fill: no reachable X11 display");
        return;
    };
    let root = connection.setup().roots[screen].clone();
    let window = connection.generate_id().unwrap();
    connection
        .create_window(
            root.root_depth,
            window,
            root.root,
            0,
            0,
            40,
            30,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new()
                .background_pixel(0x0033_6699)
                .override_redirect(1)
                .event_mask(EventMask::EXPOSURE),
        )
        .unwrap();
    connection.map_window(window).unwrap();
    connection.flush().unwrap();
    loop {
        if let Event::Expose(_) = connection.wait_for_event().unwrap() {
            break;
        }
    }
    let output = tempfile::tempdir().unwrap().keep().join("snap");
    let receipt = run(&Config {
        window: format!("0x{window:x}"),
        output: output.clone(),
    })
    .unwrap();
    assert_eq!(receipt["kind"], "x11-getimage");
    assert_eq!(receipt["width"], 40);
    assert_eq!(receipt["physical"], false);
    let frame = std::fs::read(output.join("frame.ppm")).unwrap();
    let header = b"P6\n40 30\n255\n".len();
    assert_eq!(frame.len(), header + 40 * 30 * 3);
    assert!(
        frame[header..]
            .chunks(3)
            .all(|pixel| pixel == [0x33, 0x66, 0x99])
    );
    assert!(output.join("receipt.json").exists());
    let missing = run(&Config {
        window: "0x1".to_owned(),
        output: output.with_extension("missing"),
    });
    assert!(missing.is_err());
    std::fs::remove_dir_all(output.parent().unwrap()).unwrap();
}
