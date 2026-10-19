use super::*;
use crate::linux::Desktop;
use crate::shared::{Display as _, Extent, Input as _, Phase, Point, Unit};
use x11rb::protocol;
use x11rb::protocol::xproto::{Atom, ButtonPressEvent, KeyButMask};

fn store(connection: &Connection, atom: Atom, bytes: &[u8]) {
    connection
        .conn
        .change_property8(
            PropMode::REPLACE,
            connection.root,
            atom,
            AtomEnum::STRING,
            bytes,
        )
        .unwrap()
        .check()
        .unwrap();
}

fn restore(connection: &Connection, original: Option<Vec<u8>>) {
    match original {
        Some(bytes) => store(connection, connection.atoms.resource, &bytes),
        None => {
            connection
                .conn
                .delete_property(connection.root, connection.atoms.resource)
                .unwrap()
                .check()
                .unwrap();
        }
    }
}

fn viewports(desktop: &mut Desktop<Error>, unit: Unit, wanted: usize) -> Vec<Event> {
    let mut found = Vec::new();
    for _ in 0..100 {
        found.extend(
            desktop
                .poll()
                .unwrap()
                .into_iter()
                .filter(|signal| signal.unit == unit)
                .map(|signal| signal.event)
                .filter(|event| matches!(event, Event::Viewport { .. })),
        );
        if found.len() >= wanted {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    found
}

#[test]
fn scale() {
    let Ok(external) = Connection::open() else {
        eprintln!("skip linux::window::scale: no reachable X11 display");
        return;
    };
    let original = external
        .conn
        .get_property(
            false,
            external.root,
            external.atoms.resource,
            AtomEnum::STRING,
            0,
            u32::MAX,
        )
        .unwrap()
        .reply()
        .unwrap();
    let original = (original.type_ != 0).then_some(original.value);
    let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
    let before = desktop.connection.dpi();
    let after = if before == 192 { 144 } else { 192 };
    let unit = Unit::new(11);
    desktop
        .create(
            unit,
            &Options {
                title: "Plectron native density probe".into(),
                extent: Extent {
                    width: 240,
                    height: 160,
                },
            },
        )
        .unwrap();
    desktop.activate(unit).unwrap();
    desktop.poll().unwrap();
    let probe = external
        .conn
        .intern_atom(false, b"PLECTRON_DENSITY_PROBE")
        .unwrap()
        .reply()
        .unwrap()
        .atom;
    store(&external, probe, b"Xft.dpi:\t48\n");
    let unrelated = viewports(&mut desktop, unit, 1);
    store(
        &external,
        external.atoms.resource,
        format!("Xft.dpi:\t{after}\n").as_bytes(),
    );
    let changed = viewports(&mut desktop, unit, 1);
    store(
        &external,
        external.atoms.resource,
        format!("Xft.dpi:\t{after}\n").as_bytes(),
    );
    let repeated = viewports(&mut desktop, unit, 1);
    let extent = desktop.windows[&unit].extent;
    let window = desktop.windows.get_mut(&unit).unwrap();
    let press = ButtonPressEvent {
        response_type: 4,
        detail: 1,
        sequence: 0,
        time: 5,
        root: external.root,
        event: window.xid,
        child: 0,
        root_x: 96,
        root_y: 48,
        event_x: 96,
        event_y: 48,
        state: KeyButMask::default(),
        same_screen: true,
    };
    window
        .dispatch(&protocol::Event::ButtonPress(press))
        .unwrap();
    let pointer = window.input();
    restore(&external, original);
    external
        .conn
        .delete_property(external.root, probe)
        .unwrap()
        .check()
        .unwrap();
    assert!(
        unrelated.is_empty(),
        "unrelated root property: {unrelated:?}"
    );
    let logical = |pixels: u16| (usize::from(pixels) * 96).div_ceil(after as usize);
    assert_eq!(
        changed,
        vec![Event::Viewport {
            width: logical(extent.0),
            height: logical(extent.1),
            density: (after, 96),
        }]
    );
    assert!(repeated.is_empty(), "unchanged Xft.dpi: {repeated:?}");
    assert!((desktop.windows[&unit].view.scale() - f64::from(after) / 96.0).abs() < 1e-9);
    assert!(pointer.iter().any(|event| matches!(
        event,
        Event::Pointer { position: Some(point), phase: Phase::Down(_) }
            if *point == Point { x: 96 * 96 / after as i64, y: 48 * 96 / after as i64 }
    )));
    desktop.close(unit).unwrap();
}
