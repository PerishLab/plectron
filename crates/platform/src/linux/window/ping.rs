use super::{Connection, Options, Window};
use crate::shared::Extent;
use std::rc::Rc;
use std::time::{Duration, Instant};
use x11rb::connection::Connection as _;
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, EventMask,
};
use x11rb::rust_connection::RustConnection;

fn connection() -> Option<Rc<Connection>> {
    match Connection::open() {
        Ok(connection) => Some(Rc::new(connection)),
        Err(_) => {
            eprintln!("skip linux::window::ping: no reachable X11 display");
            None
        }
    }
}

fn window(connection: Rc<Connection>) -> Window {
    Window::new(
        connection,
        &Options {
            title: "Plectron native ping probe".into(),
            extent: Extent {
                width: 64,
                height: 32,
            },
        },
    )
    .unwrap()
}

fn observe(
    observer: &RustConnection,
    root: u32,
    kind: u32,
    ping: u32,
) -> Option<ClientMessageEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match observer.poll_for_event().unwrap() {
            Some(XEvent::ClientMessage(message))
                if message.window == root
                    && message.type_ == kind
                    && message.data.as_data32().first() == Some(&ping) =>
            {
                return Some(message);
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    None
}

#[test]
fn protocols() {
    let Some(connection) = connection() else {
        return;
    };
    let window = window(connection.clone());
    let reply = connection
        .conn
        .get_property(
            false,
            window.xid,
            connection.atoms.protocols,
            AtomEnum::ATOM,
            0,
            8,
        )
        .unwrap()
        .reply()
        .unwrap();
    assert_eq!(
        reply.value32().unwrap().collect::<Vec<_>>(),
        [connection.atoms.delete, connection.atoms.ping]
    );
}

#[test]
fn pong() {
    let Some(connection) = connection() else {
        return;
    };
    let mut window = window(connection.clone());
    window.input();
    let root = connection.root;
    let (observer, _) = RustConnection::connect(None).unwrap();
    observer
        .change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
        )
        .unwrap()
        .check()
        .unwrap();
    let (kind, ping) = (connection.atoms.protocols, connection.atoms.ping);
    let message = ClientMessageEvent::new(32, window.xid, kind, [ping, 4242, window.xid, 7, 9]);
    window.dispatch(&XEvent::ClientMessage(message)).unwrap();
    let echoed = observe(&observer, root, kind, ping).expect("ping reply reaches the root window");
    assert_eq!(echoed.format, 32);
    assert_eq!(echoed.data.as_data32(), [ping, 4242, window.xid, 7, 9]);
    assert!(window.input().is_empty());
    assert!(!window.closing());
    let delete =
        ClientMessageEvent::new(32, window.xid, kind, [connection.atoms.delete, 0, 0, 0, 0]);
    window.dispatch(&XEvent::ClientMessage(delete)).unwrap();
    assert!(window.closing());
}
