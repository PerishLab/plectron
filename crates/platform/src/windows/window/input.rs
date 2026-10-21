use super::*;
use crate::shared::{Button, Event, Extent, Keyboard, Phase, Point};
use windows_sys::Win32::UI::{
    Controls::WM_MOUSELEAVE,
    Input::KeyboardAndMouse::GetCapture,
    WindowsAndMessaging::{
        SendMessageW, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_KEYDOWN, WM_KILLFOCUS,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_SETFOCUS,
    },
};

fn send(window: &Window, message: u32, data: (usize, isize)) {
    // SAFETY: the window and boxed state remain owned by this test thread throughout synchronous message delivery.
    unsafe {
        SendMessageW(window.handle, message, data.0, data.1);
    }
}
#[test]
fn route() {
    let window = Window::new(
        Rc::new(Class::new().unwrap()),
        &Options {
            title: "Plectron native input contract".into(),
            extent: Extent {
                width: 240,
                height: 160,
            },
        },
    )
    .unwrap();
    window.show().unwrap();
    window.input();
    let dpi = window.state.view.scale();
    let point = Point {
        x: (10.0 / dpi) as i64,
        y: (20.0 / dpi) as i64,
    };
    send(&window, WM_LBUTTONDOWN, (0, (20 << 16) | 10));
    // SAFETY: GetCapture reads the current test thread's window capture without borrowed pointers.
    assert_eq!(unsafe { GetCapture() }, window.handle);
    send(&window, WM_LBUTTONUP, (0, (20 << 16) | 10));
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
    send(&window, WM_SETFOCUS, (0, 0));
    send(&window, WM_KEYDOWN, (0x0d, 1 << 30));
    send(&window, WM_CHAR, (0xd83d, 0));
    send(&window, WM_CHAR, (0xde42, 0));
    let events = window.input();
    assert!(
        matches!(&events[0], Event::Input(Keyboard::Key(stroke)) if stroke.repeat && stroke.chord.code == "Enter" && stroke.text.is_empty())
    );
    assert_eq!(events[1], Event::Input(Keyboard::Text("🙂".into())));
    send(&window, WM_CHAR, (0xdc00, 0));
    assert!(window.result().is_err());
    send(&window, WM_LBUTTONDOWN, (0, 0));
    window.input();
    send(&window, WM_CAPTURECHANGED, (0, 0));
    assert_eq!(
        window.input(),
        vec![Event::Pointer {
            position: None,
            phase: Phase::Cancel
        }]
    );
    send(&window, WM_CHAR, (0xd800, 0));
    send(&window, WM_KILLFOCUS, (0, 0));
    send(&window, WM_CHAR, (65, 0));
    assert!(window.input().is_empty());
    send(&window, WM_SETFOCUS, (0, 0));
    send(&window, WM_CHAR, (65, 0));
    assert_eq!(
        window.input(),
        vec![Event::Input(Keyboard::Text("A".into()))]
    );
    send(&window, WM_CHAR, (66, 0));
    send(&window, WM_CLOSE, (0, 0));
    send(&window, WM_CHAR, (67, 0));
    assert!(window.input().is_empty());
    assert!(window.closing());
    assert!(!window.closing());
}

#[test]
fn viewport() {
    use graphics::shared::{Frame, Scene, State as Completion, Visual};
    use windows_sys::Win32::UI::{HiDpi::*, WindowsAndMessaging::*};
    // SAFETY: this reads only the current test thread's awareness context.
    let context = unsafe { GetThreadDpiAwarenessContext() };
    let window = Window::new(
        Rc::new(Class::new().unwrap()),
        &Options {
            title: "Plectron native viewport contract".into(),
            extent: Extent {
                width: 240,
                height: 160,
            },
        },
    )
    .unwrap();
    // SAFETY: both contexts were obtained on this thread and remain valid awareness identifiers.
    assert_ne!(
        unsafe { AreDpiAwarenessContextsEqual(context, GetThreadDpiAwarenessContext()) },
        0
    );
    let initial = window.input();
    assert!(initial.iter().any(|event| matches!(
        event,
        Event::Viewport {
            width: 240,
            height: 160,
            ..
        }
    )));
    let extent = graphics::shared::Extent {
        width: 240,
        height: 160,
    };
    let frame = Frame::new(
        Scene::new(extent, 0, Vec::new()).unwrap(),
        Visual::new(extent),
    )
    .unwrap();
    assert_eq!(window.draw(&frame).unwrap(), Completion::Complete);
    let scale = window.state.view.scale();
    assert_eq!(
        window.state.image.borrow().as_ref().unwrap().extent.width,
        (240.0 * scale).ceil() as usize
    );
    let _awareness = super::super::viewport::Awareness::new().unwrap();
    let dpi = if scale == 2.0 { 144 } else { 192 };
    let mut bounds = RECT {
        left: 0,
        top: 0,
        right: 640,
        bottom: 480,
    };
    // SAFETY: writable bounds and exact styles are provided for this live owned window's distinct synthetic DPI transition.
    assert_ne!(
        unsafe { AdjustWindowRectExForDpi(&mut bounds, WS_OVERLAPPEDWINDOW, 0, 0, dpi) },
        0
    );
    send(
        &window,
        WM_DPICHANGED,
        (
            ((dpi << 16) | dpi) as usize,
            (&bounds as *const RECT) as isize,
        ),
    );
    window.result().unwrap();
    let mut client = RECT::default();
    // SAFETY: client is writable and the owned HWND remains live in this per-monitor-aware test scope.
    assert_ne!(unsafe { GetClientRect(window.handle, &mut client) }, 0);
    let logical = (
        (client.right as usize * 96).div_ceil(dpi as usize),
        (client.bottom as usize * 96).div_ceil(dpi as usize),
    );
    assert!(window.input().iter().any(|event| matches!(event, Event::Viewport { width, height, density } if (*width, *height) == logical && *density == (dpi, 96))));
    window.refresh().unwrap();
    assert_eq!(
        window.state.frame.borrow().as_ref().unwrap().id(),
        frame.id()
    );
    assert_eq!(
        window.state.image.borrow().as_ref().unwrap().extent.width,
        (240 * dpi as usize).div_ceil(96)
    );
    send(&window, WM_LBUTTONDOWN, (0, 0));
    window.input();
    send(&window, WM_LBUTTONUP, (0, -1));
    assert_eq!(
        window.input(),
        vec![
            Event::Pointer {
                position: Some(Point { x: -1, y: -1 }),
                phase: Phase::Up(Button::Primary)
            },
            Event::Pointer {
                position: None,
                phase: Phase::Move
            },
        ]
    );
    send(&window, WM_CLOSE, (0, 0));
}

fn at(x: i16, y: i16) -> isize {
    ((y as u16 as isize) << 16) | x as u16 as isize
}

#[test]
fn capture() {
    let window = Window::new(
        Rc::new(Class::new().unwrap()),
        &Options {
            title: "Plectron native capture contract".into(),
            extent: Extent {
                width: 240,
                height: 160,
            },
        },
    )
    .unwrap();
    window.show().unwrap();
    window.input();
    let dpi = window.state.view.scale();
    let point = |x: i16, y: i16| Point {
        x: (f64::from(x) / dpi).floor() as i64,
        y: (f64::from(y) / dpi).floor() as i64,
    };
    let pointer = |x: i16, y: i16, phase: Phase| Event::Pointer {
        position: Some(point(x, y)),
        phase,
    };
    let leave = Event::Pointer {
        position: None,
        phase: Phase::Move,
    };
    send(&window, WM_LBUTTONDOWN, (0, at(10, 20)));
    send(&window, WM_MOUSEMOVE, (0, at(-30, -20)));
    send(&window, WM_MOUSELEAVE, (0, 0));
    send(&window, WM_LBUTTONUP, (0, at(-30, -20)));
    assert_eq!(
        window.input(),
        vec![
            pointer(10, 20, Phase::Down(Button::Primary)),
            pointer(-30, -20, Phase::Move),
            pointer(-30, -20, Phase::Up(Button::Primary)),
            leave.clone(),
        ]
    );
    send(&window, WM_LBUTTONDOWN, (0, at(10, 20)));
    send(&window, WM_MOUSEMOVE, (0, at(-30, -20)));
    send(&window, WM_MOUSEMOVE, (0, at(12, 22)));
    send(&window, WM_LBUTTONUP, (0, at(12, 22)));
    assert_eq!(
        window.input(),
        vec![
            pointer(10, 20, Phase::Down(Button::Primary)),
            pointer(-30, -20, Phase::Move),
            pointer(12, 22, Phase::Move),
            pointer(12, 22, Phase::Up(Button::Primary)),
        ]
    );
    send(&window, WM_LBUTTONDOWN, (0, at(10, 20)));
    send(&window, WM_MOUSEMOVE, (0, at(-30, -20)));
    send(&window, WM_CAPTURECHANGED, (0, 0));
    send(&window, WM_LBUTTONUP, (0, at(-30, -20)));
    assert_eq!(
        window.input(),
        vec![
            pointer(10, 20, Phase::Down(Button::Primary)),
            pointer(-30, -20, Phase::Move),
            Event::Pointer {
                position: None,
                phase: Phase::Cancel
            },
            leave,
        ]
    );
    send(&window, WM_CLOSE, (0, 0));
}
