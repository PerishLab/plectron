use ipcengine::file::{Delivery, Draft};
use std::io::{Read, Write};
use std::path::PathBuf;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "plectron-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn delivery() {
    let root = Directory::new();
    let storage = root.0.join("private");
    let mut draft = Draft::create(&storage, 3).unwrap();
    assert_eq!(std::fs::read_dir(&storage).unwrap().count(), 0);
    draft.write_all(b"abc").unwrap();
    assert!(draft.write_all(b"d").is_err());
    let artifact = draft.seal().unwrap();
    assert_eq!(artifact.length(), 3);
    let mut first = artifact.reader();
    let mut second = artifact.reader();
    drop(artifact);
    let mut bytes = [0; 2];
    first.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"ab");
    second.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"ab");
    let target = root.0.join("out");
    let mut delivery = Delivery::new(&target, 3).unwrap();
    delivery.write_all(b"a").unwrap();
    assert!(!target.exists());
    assert!(delivery.finish().is_err());
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
    let mut delivery = Delivery::new(&target, 3).unwrap();
    assert!(delivery.write_all(b"abcd").is_err());
    delivery.write_all(b"abc").unwrap();
    delivery.finish().unwrap();
    let mut duplicate = Delivery::new(&target, 3).unwrap();
    duplicate.write_all(b"xyz").unwrap();
    assert!(duplicate.finish().is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"abc");
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 2);
}

#[test]
fn readers() {
    let root = Directory::new();
    let expected: Vec<u8> = (0..65_536).map(|index| index as u8).collect();
    let mut draft = Draft::create(&root.0.join("readers"), expected.len() as u64).unwrap();
    draft.write_all(&expected).unwrap();
    let artifact = draft.seal().unwrap();
    let workers: Vec<_> = (0..4)
        .map(|index| {
            let reader = artifact.reader();
            std::thread::spawn(move || collect(reader, 257 + index))
        })
        .collect();
    drop(artifact);
    for worker in workers {
        assert_eq!(worker.join().unwrap(), expected);
    }
}
fn collect(mut reader: ipcengine::file::Reader, size: usize) -> Vec<u8> {
    let mut output = Vec::new();
    let mut bytes = vec![0; size];
    loop {
        let count = reader.read(&mut bytes).unwrap();
        if count == 0 {
            return output;
        }
        output.extend_from_slice(&bytes[..count]);
        std::thread::yield_now();
    }
}
