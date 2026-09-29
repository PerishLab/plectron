use ipcengine::{
    buffer::Buffer,
    message::Connection,
    stream::{Duplex, State, Transfer},
};
use std::io::{Cursor, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[test]
fn framing() {
    let (mut producer, consumer) = UnixStream::pair().unwrap();
    producer.write_all(b"12\n34\n").unwrap();
    let mut connection = Connection::new(consumer).unwrap();
    assert_eq!(
        connection.read::<u32>(2, Duration::from_secs(1)).unwrap(),
        12
    );
    assert_eq!(
        connection.read::<u32>(2, Duration::from_secs(1)).unwrap(),
        34
    );
    assert!(connection.write(&"too long", 2).is_err());
    connection.write(&42, 2).unwrap();
    let mut reply = [0; 3];
    producer.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"42\n");
    producer.write_all(b"123\n").unwrap();
    assert!(connection.read::<u32>(2, Duration::from_secs(1)).is_err());
    drop(producer);
    assert!(connection.write(&42, 2).is_err());
    assert!(
        connection
            .write(&42, 2)
            .unwrap_err()
            .to_string()
            .contains("previous write")
    );
}
#[test]
fn duplex() {
    let (mut left, mut right) = Duplex::pair().unwrap();
    let mut bytes = [0; 3];
    assert_eq!(
        right.read(&mut bytes).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    left.write_all(b"abc").unwrap();
    left.finish().unwrap();
    right.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"abc");
    assert_eq!(right.read(&mut bytes).unwrap(), 0);
    right.write_all(b"yes").unwrap();
    left.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"yes");
    left.cancel();
    assert!(right.write(b"x").is_err());
}
#[test]
fn pressure() {
    let (mut left, mut right) = Duplex::pair().unwrap();
    let bytes = [42; 65_536];
    let mut written = 0;
    loop {
        match left.write(&bytes) {
            Ok(count) => written += count,
            Err(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
                break;
            }
        }
        assert!(written < 16_777_216);
    }
    let mut received = vec![0; written];
    right.read_exact(&mut received).unwrap();
    assert!(received.iter().all(|byte| *byte == 42));
    assert!(left.write(b"next").is_ok());
}
struct Source {
    cursor: Cursor<Vec<u8>>,
    drops: Arc<AtomicUsize>,
}
impl Read for Source {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(bytes)
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn transfer() {
    let drops = Arc::new(AtomicUsize::new(0));
    let source = Source {
        cursor: Cursor::new(vec![1, 2, 3]),
        drops: drops.clone(),
    };
    let mut transfer = Transfer::new(source, 3);
    assert!(transfer.read(0).is_err());
    assert!(transfer.read(65_537).is_err());
    assert_eq!(transfer.read(2).unwrap().bytes, [1, 2]);
    assert_eq!(transfer.state(), State::Open);
    let chunk = transfer.read(2).unwrap();
    assert_eq!(chunk.bytes, [3]);
    assert!(chunk.eof);
    assert_eq!(transfer.state(), State::Ended);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    transfer.close();
    assert!(transfer.read(1).is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    transfer.close();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let mut short = Transfer::new(Cursor::new([1]), 2);
    assert!(short.read(2).is_err());
    assert_eq!(short.state(), State::Failed);
}
#[test]
fn publication() {
    let mut buffer = Buffer::new(3).unwrap();
    buffer.writable().unwrap().copy_from_slice(b"one");
    let first = buffer.publish().unwrap();
    let lease = buffer.borrow(first).unwrap();
    assert!(buffer.writable().is_err());
    assert!(buffer.reclaim(first).is_err());
    assert_eq!(lease.bytes(), b"one");
    assert_eq!(lease.generation(), first);
    drop(lease);
    buffer.reclaim(first).unwrap();
    buffer.writable().unwrap().copy_from_slice(b"two");
    let second = buffer.publish().unwrap();
    assert!(buffer.borrow(first).is_err());
    let lease = buffer.borrow(second).unwrap();
    drop(buffer);
    assert_eq!(lease.bytes(), b"two");
}

#[test]
fn disconnect() {
    use ipcengine::{observe::Monitor, stream::Client};
    for cancel in [true, false] {
        let drops = Arc::new(AtomicUsize::new(0));
        let source = Source {
            cursor: Cursor::new(vec![1, 2, 3]),
            drops: drops.clone(),
        };
        let (producer, consumer) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            Transfer::new(source, 3).serve(&mut Connection::new(producer).unwrap());
        });
        let mut connection = Connection::new(consumer).unwrap();
        let monitor = Monitor::default();
        let mut client = Client::new(&mut connection, 3, &monitor);
        assert_eq!(client.read(1).unwrap().bytes, [1]);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        if !cancel {
            client.close().unwrap();
        } else {
            drop(client);
        }
        drop(connection);
        worker.join().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
