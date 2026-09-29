use ipcengine::{
    message::Connection,
    observe::Monitor,
    stream::{Client, Transfer},
};
use std::io::{Cursor, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn frame(kind: u8, length: u64) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes[..4].copy_from_slice(b"PLS2");
    bytes[4] = kind;
    bytes[8..].copy_from_slice(&length.to_be_bytes());
    bytes
}
#[test]
fn retained() {
    let (producer, consumer) = UnixStream::pair().unwrap();
    let bytes: Vec<u8> = (0..100_000).map(|index| index as u8).collect();
    let expected = bytes.clone();
    let transfer = Transfer::new(Cursor::new(bytes), 100_000);
    let source = transfer.monitor();
    let worker =
        std::thread::spawn(move || transfer.serve(&mut Connection::new(producer).unwrap()));
    let mut connection = Connection::new(consumer).unwrap();
    let monitor = Monitor::default();
    let mut client = Client::new(&mut connection, 100_000, &monitor);
    let mut actual = client.read(65_536).unwrap().bytes;
    let chunk = client.read(65_536).unwrap();
    assert!(chunk.eof);
    actual.extend(chunk.bytes);
    assert_eq!(expected, actual);
    assert_eq!(monitor.snapshot().active, 1);
    client.close().unwrap();
    worker.join().unwrap();
    for snapshot in [monitor.snapshot(), source.snapshot()] {
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.bytes, 100_000);
        assert_eq!(snapshot.completed, 1);
        assert!(snapshot.first.is_some());
        assert!(snapshot.waits > 0);
    }
}
#[test]
fn coalesced() {
    let (mut producer, consumer) = UnixStream::pair().unwrap();
    let mut bytes = b"true\n".to_vec();
    bytes.extend(frame(2, 3));
    bytes.extend([0, 10, 255]);
    bytes.extend(frame(3, 0));
    producer.write_all(&bytes).unwrap();
    let mut connection = Connection::new(consumer).unwrap();
    assert!(connection.read::<bool>(8, Duration::from_secs(1)).unwrap());
    let monitor = Monitor::default();
    let mut client = Client::new(&mut connection, 3, &monitor);
    assert_eq!(client.read(3).unwrap().bytes, [0, 10, 255]);
    client.close().unwrap();
    assert_eq!(monitor.snapshot().completed, 1);
}
#[test]
fn malformed() {
    let cases = [
        (2, u64::MAX, vec![]),
        (2, 3, vec![1]),
        (1, 0, vec![]),
        (9, 0, vec![]),
    ];
    for (kind, length, body) in cases {
        let (mut producer, consumer) = UnixStream::pair().unwrap();
        producer.write_all(&frame(kind, length)).unwrap();
        producer.write_all(&body).unwrap();
        producer.shutdown(std::net::Shutdown::Write).unwrap();
        let mut connection = Connection::new(consumer).unwrap();
        let monitor = Monitor::default();
        let mut client = Client::new(&mut connection, 3, &monitor);
        assert!(client.read(3).is_err());
        assert!(client.read(3).is_err());
        drop(client);
        assert_eq!(monitor.snapshot().failed, 1);
        assert_eq!(monitor.snapshot().active, 0);
    }
}
#[test]
fn empty() {
    let (producer, consumer) = UnixStream::pair().unwrap();
    let worker = std::thread::spawn(move || {
        Transfer::new(Cursor::new([]), 0).serve(&mut Connection::new(producer).unwrap())
    });
    let mut connection = Connection::new(consumer).unwrap();
    let monitor = Monitor::default();
    let mut client = Client::new(&mut connection, 0, &monitor);
    let chunk = client.read(1).unwrap();
    assert!(chunk.bytes.is_empty() && chunk.eof);
    client.close().unwrap();
    worker.join().unwrap();
    assert_eq!(monitor.snapshot().completed, 1);
    assert_eq!(monitor.snapshot().first, None);
}
