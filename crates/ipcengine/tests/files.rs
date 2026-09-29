use ipcengine::file::{Delivery, Draft};
use std::io::{Read, Write};
use std::path::PathBuf;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("plectron-files-{}", std::process::id()));
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
