mod process;
use ipcengine::{endpoint, file::Delivery, message::Connection, observe::Monitor, stream::Client};
use std::{io::Write, time::Duration};

#[test]
fn exchange() {
    let child = process::Child::start();
    let path = endpoint::path(&child.root, child.pid);
    let mut connection = Connection::connect(&path).unwrap();
    connection.write(&"file", 64).unwrap();
    let length: u64 = connection.read(64, Duration::from_secs(2)).unwrap();
    assert_eq!(length, 3);
    let mut client = Client::new(&mut connection, length, &Monitor::default());
    let target = child.root.join("output");
    let mut delivery = Delivery::new(&target, length).unwrap();
    let first = client.read(2).unwrap();
    assert_eq!(first.bytes, b"ab");
    assert!(!first.eof);
    delivery.write_all(&first.bytes).unwrap();
    let last = client.read(2).unwrap();
    assert_eq!(last.bytes, b"c");
    assert!(last.eof);
    delivery.write_all(&last.bytes).unwrap();
    client.close().unwrap();
    delivery.finish().unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"abc");
    drop(connection);
    child.stop();
}

#[test]
fn deadlines() {
    let child = process::Child::start();
    let path = endpoint::path(&child.root, child.pid);
    let mut idle = Connection::connect(&path).unwrap();
    let started = std::time::Instant::now();
    assert!(idle.read::<u32>(64, Duration::from_millis(30)).is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(idle);
    child.stop();
}

#[test]
#[ignore = "separate-process IPC service selected explicitly by the parent tests"]
fn service() {
    process::serve();
}

#[test]
fn shutdown() {
    let child = process::Child::start();
    let mut idle = Connection::connect(&endpoint::path(&child.root, child.pid)).unwrap();
    let started = std::time::Instant::now();
    child.stop();
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(idle.read::<u32>(64, Duration::from_millis(30)).is_err());
}
