use super::super::{Error, connection::Connection, error};
use x11rb::protocol::xproto::{AtomEnum, PropMode, Window as Xid};
use x11rb::wrapper::ConnectionExt as _;

pub(super) fn publish(connection: &Connection, xid: Xid) -> Result<(), Error> {
    let class = b"plectron\0Plectron\0";
    connection
        .conn
        .change_property8(
            PropMode::REPLACE,
            xid,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            class,
        )
        .map_err(error)?
        .check()
        .map_err(error)?;
    connection
        .conn
        .change_property32(
            PropMode::REPLACE,
            xid,
            connection.atoms.pid,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )
        .map_err(error)?
        .check()
        .map_err(error)?;
    let machine = machine()?;
    connection
        .conn
        .change_property8(
            PropMode::REPLACE,
            xid,
            AtomEnum::WM_CLIENT_MACHINE,
            AtomEnum::STRING,
            &machine,
        )
        .map_err(error)?
        .check()
        .map_err(error)?;
    Ok(())
}

fn machine() -> Result<Vec<u8>, Error> {
    for path in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(text) = std::fs::read_to_string(path) {
            let name = text.trim_end_matches(['\n', '\r']);
            if !name.is_empty() && !name.as_bytes().contains(&0) {
                return Ok(name.as_bytes().to_vec());
            }
        }
    }
    Err(Error("X11 client machine name is absent".into()))
}

#[cfg(test)]
mod tests {
    use super::super::super::connection::Connection;
    use super::super::{Options, Window};
    use crate::shared::Extent;
    use std::rc::Rc;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

    fn display() -> bool {
        let available = Connection::open().is_ok();
        if !available {
            eprintln!("skip linux::window::identity: no reachable X11 display");
        }
        available
    }

    #[test]
    fn properties() {
        if !display() {
            return;
        }
        let connection = Rc::new(Connection::open().unwrap());
        let window = Window::new(
            connection.clone(),
            &Options {
                title: "Plectron identity probe".into(),
                extent: Extent {
                    width: 64,
                    height: 32,
                },
            },
        )
        .unwrap();
        let class = connection
            .conn
            .get_property(
                false,
                window.xid,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                0,
                64,
            )
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(class.value, b"plectron\0Plectron\0");
        let pid = connection
            .conn
            .get_property(
                false,
                window.xid,
                connection.atoms.pid,
                AtomEnum::CARDINAL,
                0,
                1,
            )
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(
            pid.value32().unwrap().collect::<Vec<_>>(),
            [std::process::id()]
        );
        let machine = connection
            .conn
            .get_property(
                false,
                window.xid,
                AtomEnum::WM_CLIENT_MACHINE,
                AtomEnum::STRING,
                0,
                256,
            )
            .unwrap()
            .reply()
            .unwrap();
        assert!(!machine.value.is_empty());
        assert!(!machine.value.contains(&0));
        let expected = super::machine().unwrap();
        assert_eq!(machine.value, expected);
    }
}
