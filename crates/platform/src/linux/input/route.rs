use super::*;
use crate::shared::{Button, Point};
use x11rb::protocol::xproto::{
    ButtonPressEvent, EnterNotifyEvent, FocusInEvent, KeyButMask, KeyPressEvent,
};

fn focus(detail: NotifyDetail, mode: NotifyMode) -> FocusInEvent {
    FocusInEvent {
        response_type: 9,
        detail,
        sequence: 0,
        event: 2,
        mode,
    }
}
fn key(detail: u8) -> KeyPressEvent {
    KeyPressEvent {
        response_type: 2,
        detail,
        sequence: 0,
        time: 1,
        root: 1,
        event: 2,
        child: 0,
        root_x: 0,
        root_y: 0,
        event_x: 0,
        event_y: 0,
        state: KeyButMask::default(),
        same_screen: true,
    }
}
fn press(x: i16, y: i16) -> ButtonPressEvent {
    ButtonPressEvent {
        response_type: 4,
        detail: 1,
        sequence: 0,
        time: 3,
        root: 1,
        event: 2,
        child: 0,
        root_x: x,
        root_y: y,
        event_x: x,
        event_y: y,
        state: KeyButMask::default(),
        same_screen: true,
    }
}
fn leave(mode: NotifyMode) -> EnterNotifyEvent {
    EnterNotifyEvent {
        response_type: 8,
        detail: NotifyDetail::ANCESTOR,
        sequence: 0,
        time: 4,
        root: 1,
        event: 2,
        child: 0,
        root_x: 0,
        root_y: 0,
        event_x: 0,
        event_y: 0,
        state: KeyButMask::default(),
        mode,
        same_screen_focus: 0,
    }
}

#[test]
fn bound() {
    let queue = Queue::default();
    for _ in 0..512 {
        queue.push(Event::Scroll { delta: 1 }).unwrap();
    }
    assert!(queue.push(Event::Scroll { delta: 1 }).is_err());
    assert_eq!(queue.drain().len(), 16);
    assert!(queue.pending());
    queue.close();
    queue.push(Event::Scroll { delta: 1 }).unwrap();
    assert!(!queue.pending());
}

#[test]
fn merge() {
    let queue = Queue::default();
    for width in [10, 20] {
        queue
            .push(Event::Viewport {
                width,
                height: 5,
                density: (96, 96),
            })
            .unwrap();
    }
    for x in [1, 2] {
        queue
            .push(Event::Pointer {
                position: Some(Point { x, y: 0 }),
                phase: Phase::Move,
            })
            .unwrap();
    }
    assert_eq!(
        queue.drain(),
        vec![
            Event::Viewport {
                width: 20,
                height: 5,
                density: (96, 96),
            },
            Event::Pointer {
                position: Some(Point { x: 2, y: 0 }),
                phase: Phase::Move,
            },
        ]
    );
}

#[test]
fn route() {
    let queue = Queue::default();
    queue.remap(8, 2, vec![0x61, 0x41]).unwrap();
    let extent = (64, 32);
    queue.dispatch(&XEvent::KeyPress(key(8)), extent).unwrap();
    assert!(!queue.pending());
    queue
        .dispatch(
            &XEvent::FocusIn(focus(NotifyDetail::INFERIOR, NotifyMode::NORMAL)),
            extent,
        )
        .unwrap();
    queue.dispatch(&XEvent::KeyPress(key(8)), extent).unwrap();
    assert!(!queue.pending());
    queue
        .dispatch(
            &XEvent::FocusIn(focus(NotifyDetail::NONLINEAR, NotifyMode::NORMAL)),
            extent,
        )
        .unwrap();
    queue.dispatch(&XEvent::KeyPress(key(8)), extent).unwrap();
    assert_eq!(queue.drain().len(), 2);
    assert_eq!(
        queue
            .dispatch(&XEvent::ButtonPress(press(4, 6)), extent)
            .unwrap(),
        Effect::Focus(3)
    );
    queue
        .dispatch(&XEvent::LeaveNotify(leave(NotifyMode::NORMAL)), extent)
        .unwrap();
    queue
        .dispatch(&XEvent::LeaveNotify(leave(NotifyMode::GRAB)), extent)
        .unwrap();
    queue
        .dispatch(&XEvent::LeaveNotify(leave(NotifyMode::GRAB)), extent)
        .unwrap();
    queue
        .dispatch(&XEvent::ButtonPress(press(4, 6)), extent)
        .unwrap();
    queue
        .dispatch(
            &XEvent::FocusOut(focus(NotifyDetail::NONLINEAR, NotifyMode::GRAB)),
            extent,
        )
        .unwrap();
    queue.dispatch(&XEvent::KeyPress(key(8)), extent).unwrap();
    let down = Event::Pointer {
        position: Some(Point { x: 4, y: 6 }),
        phase: Phase::Down(Button::Primary),
    };
    let cancel = Event::Pointer {
        position: None,
        phase: Phase::Cancel,
    };
    assert_eq!(
        queue.drain(),
        vec![down.clone(), cancel.clone(), down, cancel]
    );
    queue.close();
    queue
        .dispatch(
            &XEvent::FocusIn(focus(NotifyDetail::NONLINEAR, NotifyMode::NORMAL)),
            extent,
        )
        .unwrap();
    queue.dispatch(&XEvent::KeyPress(key(8)), extent).unwrap();
    assert!(!queue.pending());
}

#[test]
fn resize() {
    let queue = Queue::default();
    let mut view = crate::linux::viewport::View::new(192);
    let pixels = view
        .initialize(
            crate::shared::Extent {
                width: 320,
                height: 200,
            },
            &queue,
        )
        .unwrap();
    assert_eq!(pixels, (640, 400));
    assert_eq!(
        queue.drain(),
        vec![Event::Viewport {
            width: 320,
            height: 200,
            density: (192, 96),
        }]
    );
    view.configure(640, 400, &queue).unwrap();
    assert!(!view.changed());
    view.configure(0, 400, &queue).unwrap();
    view.configure(801, 600, &queue).unwrap();
    view.configure(1001, 700, &queue).unwrap();
    assert!(view.changed());
    assert_eq!(
        queue.drain(),
        vec![Event::Viewport {
            width: 501,
            height: 350,
            density: (192, 96),
        }]
    );
    assert!(!queue.pending());
}
