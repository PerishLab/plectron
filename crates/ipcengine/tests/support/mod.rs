use ipcengine::transport::Socket;

#[cfg(unix)]
pub fn pair() -> std::io::Result<(Socket, Socket)> {
    Socket::pair()
}

#[cfg(windows)]
pub fn pair() -> std::io::Result<(Socket, Socket)> {
    use ipcengine::{endpoint, transport::Listener};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "plectron-pair-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    endpoint::prepare(&root).map_err(std::io::Error::other)?;
    let path = endpoint::path(&root, std::process::id());
    let listener = Listener::bind(&path)?;
    let client = Socket::connect(&path)?;
    let server = listener.accept()?;
    drop(listener);
    std::fs::remove_dir(&root)?;
    Ok((server, client))
}
