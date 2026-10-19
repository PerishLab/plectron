use super::*;
use crate::shared::{Button, Extent, Keyboard, Phase, Point, Unit};
use x11rb::protocol;
use x11rb::protocol::xproto::{
    ButtonPressEvent, ClientMessageEvent, ConfigureNotifyEvent, DestroyNotifyEvent, FocusInEvent,
    KeyButMask, KeyPressEvent, NotifyDetail, NotifyMode,
};

fn connection(name: &str) -> Option<Rc<Connection>> {
    match Connection::open() {
        Ok(connection) => Some(Rc::new(connection)),
        Err(_) => {
            eprintln!("skip linux::window::input::{name}: no reachable X11 display");
            None
        }
    }
}
fn window(connection: Rc<Connection>, title: &str) -> Window {
    let window = Window::new(
        connection,
        &Options {
            title: title.into(),
            extent: Extent {
                width: 240,
                height: 160,
            },
        },
    )
    .unwrap();
    window.remap(8, 2, vec![0x61, 0x41, 0xff0d, 0]).unwrap();
    window
}
fn button(window: &Window, release: bool, x: i16, y: i16) -> protocol::Event {
    let event = ButtonPressEvent {
        response_type: if release { 5 } else { 4 },
        detail: 1,
        sequence: 0,
        time: 3,
        root: window.connection.root,
        event: window.xid,
        child: 0,
        root_x: x,
        root_y: y,
        event_x: x,
        event_y: y,
        state: KeyButMask::default(),
        same_screen: true,
    };
    if release {
        protocol::Event::ButtonRelease(event)
    } else {
        protocol::Event::ButtonPress(event)
    }
}
fn key(window: &Window, release: bool, (detail, state, time): (u8, u16, u32)) -> protocol::Event {
    let event = KeyPressEvent {
        response_type: if release { 3 } else { 2 },
        detail,
        sequence: 0,
        time,
        root: window.connection.root,
        event: window.xid,
        child: 0,
        root_x: 0,
        root_y: 0,
        event_x: 0,
        event_y: 0,
        state: state.into(),
        same_screen: true,
    };
    if release {
        protocol::Event::KeyRelease(event)
    } else {
        protocol::Event::KeyPress(event)
    }
}
fn focus(window: &Window, inside: bool) -> protocol::Event {
    let event = FocusInEvent {
        response_type: if inside { 9 } else { 10 },
        detail: NotifyDetail::NONLINEAR,
        sequence: 0,
        event: window.xid,
        mode: NotifyMode::NORMAL,
    };
    if inside {
        protocol::Event::FocusIn(event)
    } else {
        protocol::Event::FocusOut(event)
    }
}
fn stroke(code: &str, control: bool, repeat: bool) -> Event {
    Event::Input(Keyboard::Key(shared::shortcut::Stroke {
        chord: shared::shortcut::Chord {
            code: code.into(),
            modifiers: shared::shortcut::Modifiers {
                control,
                ..Default::default()
            },
        },
        repeat,
        text: String::new(),
    }))
}

#[test]
fn route() {
    let Some(connection) = connection("route") else {
        return;
    };
    let dpi = u64::from(connection.dpi());
    let mut window = window(connection, "Plectron native input route");
    window.input();
    let point = Point {
        x: (10 * 96 / dpi) as i64,
        y: (20 * 96 / dpi) as i64,
    };
    window.dispatch(&button(&window, false, 10, 20)).unwrap();
    window.dispatch(&button(&window, true, 10, 20)).unwrap();
    assert_eq!(
        window.input(),
        vec![
            Event::Pointer {
                position: Some(point),
                phase: Phase::Down(Button::Primary)
            },
            Event::Pointer {
                position: Some(point),
                phase: Phase::Up(Button::Primary)
            },
        ]
    );
    window.dispatch(&key(&window, false, (9, 0, 10))).unwrap();
    assert!(window.input().is_empty());
    window.dispatch(&focus(&window, true)).unwrap();
    for (release, stroke) in [
        (false, (9, 0, 10)),
        (true, (9, 0, 40)),
        (false, (9, 0, 40)),
        (true, (9, 0, 50)),
        (false, (8, 4, 60)),
        (true, (8, 4, 70)),
        (false, (8, 0, 90)),
    ] {
        window.dispatch(&key(&window, release, stroke)).unwrap();
    }
    assert_eq!(
        window.input(),
        vec![
            stroke("Enter", false, false),
            stroke("Enter", false, true),
            stroke("KeyA", true, false),
            stroke("KeyA", false, false),
            Event::Input(Keyboard::Text("a".into())),
        ]
    );
    window.dispatch(&button(&window, false, 0, 0)).unwrap();
    window.input();
    window.dispatch(&focus(&window, false)).unwrap();
    window.dispatch(&key(&window, false, (8, 0, 100))).unwrap();
    assert_eq!(
        window.input(),
        vec![Event::Pointer {
            position: None,
            phase: Phase::Cancel
        }]
    );
    window.dispatch(&focus(&window, true)).unwrap();
    window.dispatch(&key(&window, false, (8, 0, 110))).unwrap();
    let delete = ClientMessageEvent::new(
        32,
        window.xid,
        window.connection.atoms.protocols,
        [window.connection.atoms.delete, 0, 0, 0, 0],
    );
    window
        .dispatch(&protocol::Event::ClientMessage(delete))
        .unwrap();
    window.dispatch(&key(&window, false, (8, 0, 120))).unwrap();
    assert!(window.input().is_empty());
    assert!(window.closing());
    assert!(!window.closing());
}

#[test]
fn viewport() {
    use graphics::shared::{Compute, Frame, Scene, State as Completion, Visual};
    let Some(connection) = connection("viewport") else {
        return;
    };
    let dpi = connection.dpi();
    let compute = Compute::new(shared::Notify::thread(std::thread::current())).unwrap();
    let mut window = window(connection, "Plectron native input viewport");
    assert!(window.input().contains(&Event::Viewport {
        width: 240,
        height: 160,
        density: (dpi, 96),
    }));
    let extent = graphics::shared::Extent {
        width: 240,
        height: 160,
    };
    let frame = Frame::new(
        Scene::new(extent, 0, Vec::new()).unwrap(),
        Visual::new(extent),
    )
    .unwrap();
    let unit = Unit::new(3);
    assert_eq!(window.draw(&frame, &compute).unwrap(), Completion::Pending);
    let mut receipts = Vec::new();
    for _ in 0..500 {
        receipts.extend(window.completed(unit, &compute));
        if !receipts.is_empty() {
            break;
        }
        std::thread::park_timeout(std::time::Duration::from_millis(10));
    }
    assert_eq!(receipts.len(), 1);
    assert_eq!((receipts[0].unit, receipts[0].frame), (unit, frame.id()));
    assert!(receipts[0].result.is_ok() && receipts[0].produced.is_some());
    assert!(window.image.is_some());
    let configure = ConfigureNotifyEvent {
        response_type: 22,
        sequence: 0,
        event: window.xid,
        window: window.xid,
        above_sibling: 0,
        x: 0,
        y: 0,
        width: 640,
        height: 480,
        border_width: 0,
        override_redirect: false,
    };
    window
        .dispatch(&protocol::Event::ConfigureNotify(configure))
        .unwrap();
    let logical = (
        (640 * 96usize).div_ceil(dpi as usize),
        (480 * 96usize).div_ceil(dpi as usize),
    );
    assert_eq!(
        window.input(),
        vec![Event::Viewport {
            width: logical.0,
            height: logical.1,
            density: (dpi, 96),
        }]
    );
    window.refresh(&compute).unwrap();
    assert_eq!(window.raster.frame().map(Frame::id), Some(frame.id()));
    assert!(!window.raster.pending());
    assert!(window.completed(unit, &compute).is_empty());
    assert!(!window.view.changed());
    window.dispatch(&button(&window, false, 0, 0)).unwrap();
    window.dispatch(&button(&window, true, 600, 10)).unwrap();
    window.dispatch(&button(&window, false, 0, 0)).unwrap();
    window.dispatch(&button(&window, true, 700, 10)).unwrap();
    let phases = window.input().into_iter().map(|event| match event {
        Event::Pointer { phase, .. } => phase,
        _ => Phase::Move,
    });
    let down = Phase::Down(Button::Primary);
    let up = Phase::Up(Button::Primary);
    let expected = [down, up, down, up];
    assert!(phases.eq(expected));
    let destroy = DestroyNotifyEvent {
        response_type: 17,
        sequence: 0,
        event: window.xid,
        window: window.xid,
    };
    window
        .dispatch(&protocol::Event::DestroyNotify(destroy))
        .unwrap();
    assert!(window.closing());
    assert!(window.title("destroyed").is_err());
}
