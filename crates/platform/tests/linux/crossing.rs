#![cfg(test)]

use super::Queue;
use crate::shared::{Button, Event, Phase, Point};
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{
    ButtonPressEvent, EnterNotifyEvent, FocusInEvent, KeyButMask, Motion, MotionNotifyEvent,
    NotifyDetail, NotifyMode,
};

const OUT: Event = Event::Pointer {
    position: None,
    phase: Phase::Move,
};

fn pointer(x: i64, y: i64, phase: Phase) -> Event {
    Event::Pointer {
        position: Some(Point { x, y }),
        phase,
    }
}
fn cross(detail: NotifyDetail, mode: NotifyMode, x: i16, y: i16) -> EnterNotifyEvent {
    EnterNotifyEvent {
        response_type: 7,
        detail,
        sequence: 0,
        time: 4,
        root: 1,
        event: 2,
        child: 0,
        root_x: x,
        root_y: y,
        event_x: x,
        event_y: y,
        state: KeyButMask::default(),
        mode,
        same_screen_focus: 0,
    }
}
fn enter(x: i16, y: i16) -> XEvent {
    XEvent::EnterNotify(cross(NotifyDetail::NONLINEAR, NotifyMode::NORMAL, x, y))
}
fn leave(mode: NotifyMode) -> XEvent {
    XEvent::LeaveNotify(cross(NotifyDetail::ANCESTOR, mode, 0, 0))
}
fn button(x: i16, y: i16) -> ButtonPressEvent {
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
fn moved(x: i16, y: i16) -> XEvent {
    XEvent::MotionNotify(MotionNotifyEvent {
        response_type: 6,
        detail: Motion::NORMAL,
        sequence: 1,
        time: 9,
        root: 1,
        event: 2,
        child: 0,
        root_x: x,
        root_y: y,
        event_x: x,
        event_y: y,
        state: KeyButMask::BUTTON1,
        same_screen: true,
    })
}
fn run(queue: &Queue, events: &[XEvent]) -> Vec<Event> {
    for event in events {
        queue.dispatch(event).unwrap();
    }
    queue.drain()
}

#[test]
fn exit() {
    let queue = Queue::default();
    assert_eq!(
        run(&queue, &[enter(10, 20)]),
        vec![pointer(10, 20, Phase::Move)]
    );
    let inferior = cross(NotifyDetail::INFERIOR, NotifyMode::NORMAL, 10, 20);
    let events = [XEvent::LeaveNotify(inferior), XEvent::EnterNotify(inferior)];
    assert_eq!(run(&queue, &events), vec![]);
    let events = [leave(NotifyMode::NORMAL), leave(NotifyMode::NORMAL)];
    assert_eq!(run(&queue, &events), vec![OUT]);
    let events = [leave(NotifyMode::GRAB), leave(NotifyMode::UNGRAB)];
    assert_eq!(run(&queue, &events), vec![]);
    let events = [enter(3, 4), leave(NotifyMode::GRAB)];
    assert_eq!(run(&queue, &events), vec![OUT]);
}

#[test]
fn capture() {
    let queue = Queue::default();
    let down = Phase::Down(Button::Primary);
    let up = Phase::Up(Button::Primary);
    let outside = [
        enter(50, 50),
        XEvent::ButtonPress(button(50, 50)),
        leave(NotifyMode::NORMAL),
        moved(-40, -25),
        XEvent::ButtonRelease(button(-40, -25)),
        leave(NotifyMode::UNGRAB),
    ];
    assert_eq!(
        run(&queue, &outside),
        vec![
            pointer(50, 50, Phase::Move),
            pointer(50, 50, down),
            pointer(-40, -25, Phase::Move),
            pointer(-40, -25, up),
            OUT,
        ]
    );
    let back = [
        enter(60, 60),
        XEvent::ButtonPress(button(60, 60)),
        leave(NotifyMode::NORMAL),
        moved(-5, -5),
        enter(7, 8),
        XEvent::ButtonRelease(button(7, 8)),
    ];
    assert_eq!(
        run(&queue, &back),
        vec![
            pointer(60, 60, Phase::Move),
            pointer(60, 60, down),
            pointer(7, 8, Phase::Move),
            pointer(7, 8, up),
        ]
    );
}

#[test]
fn loss() {
    let queue = Queue::default();
    let cancel = Event::Pointer {
        position: None,
        phase: Phase::Cancel,
    };
    let focus = FocusInEvent {
        response_type: 10,
        detail: NotifyDetail::NONLINEAR,
        sequence: 0,
        event: 2,
        mode: NotifyMode::NORMAL,
    };
    let lost = [
        XEvent::ButtonPress(button(5, 5)),
        leave(NotifyMode::NORMAL),
        XEvent::FocusOut(focus),
    ];
    assert_eq!(
        run(&queue, &lost),
        vec![
            pointer(5, 5, Phase::Down(Button::Primary)),
            cancel.clone(),
            OUT
        ]
    );
    let grabbed = [
        enter(5, 5),
        XEvent::ButtonPress(button(5, 5)),
        leave(NotifyMode::GRAB),
    ];
    assert_eq!(
        run(&queue, &grabbed),
        vec![
            pointer(5, 5, Phase::Move),
            pointer(5, 5, Phase::Down(Button::Primary)),
            cancel,
            OUT,
        ]
    );
}
