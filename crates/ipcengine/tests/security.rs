use ipcengine::{endpoint, transport::Listener};
use std::path::PathBuf;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "plectron-security-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn private() {
    let root = Directory::new();
    endpoint::prepare(&root.0).unwrap();
    endpoint::prepare(&root.0).unwrap();
    let path = endpoint::path(&root.0, std::process::id());
    let listener = Listener::bind(&path).unwrap();
    assert!(Listener::bind(&path).is_err());
    drop(listener);
    assert!(!path.exists());
}
#[test]
fn ordinary() {
    let root = Directory::new();
    std::fs::create_dir(&root.0).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(endpoint::prepare(&root.0).is_err());
}
#[test]
fn file() {
    let root = Directory::new();
    std::fs::create_dir(&root.0).unwrap();
    let path = root.0.join("file");
    std::fs::write(&path, b"sentinel").unwrap();
    assert!(endpoint::prepare(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"sentinel");
}
