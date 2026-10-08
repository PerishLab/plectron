use plectron::resource::Services;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use webengine::resource::{Provider, address};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Root(PathBuf);

impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "plectron-static-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("index.html"), "<body>hello</body>").unwrap();
        Self(path)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn serving() {
    let root = Root::new();
    let mut services = Services::default();
    let id = services.serve(&root.0).unwrap();
    services.scheme(id, "app://demo/").unwrap();
    assert!(services.scheme(id, "app://demo/").is_err());
    assert!(services.scheme(id, "http://demo/").is_err());
    let url = address("app://demo/index.html").unwrap();
    let response = services.get(&url).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.mime, "text/html");
    assert_eq!(response.body, b"<body>hello</body>");
    assert_eq!(
        services
            .get(&url.join("missing.css").unwrap())
            .unwrap()
            .status,
        404
    );
    assert!(
        services
            .get(&address("app://other/index.html").unwrap())
            .is_err()
    );
    let endpoint = services.inspect()[0].address.clone();
    services.close(id).unwrap();
    assert!(services.get(&url).is_err());
    assert!(TcpStream::connect(endpoint.strip_prefix("http://").unwrap()).is_err());
}

#[test]
fn confinement() {
    let root = Root::new();
    std::os::unix::fs::symlink(root.0.parent().unwrap(), root.0.join("escape")).unwrap();
    let mut services = Services::default();
    let id = services.serve(&root.0).unwrap();
    services.scheme(id, "app://demo/").unwrap();
    let endpoint = services.inspect()[0].address.clone();
    for path in ["/../index.html", "/%2e%2e/index.html", "/escape", "/%00"] {
        let mut stream = TcpStream::connect(endpoint.strip_prefix("http://").unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 403"), "{response}");
    }
}

#[test]
fn modules() {
    let root = Root::new();
    std::fs::create_dir(root.0.join("packages")).unwrap();
    std::fs::write(root.0.join("pnpm-workspace.yaml"), "packages: []\n").unwrap();
    std::fs::write(root.0.join("main.ts"),
        "import type {Missing} from './absent.ts'; import {value} from './value.ts'; const result: number = value;").unwrap();
    std::fs::write(root.0.join("value.ts"), "export const value: number = 7;").unwrap();
    std::fs::write(
        root.0.join("main.js"),
        "import {value} from './value.ts'; const result = value;",
    )
    .unwrap();
    std::fs::write(root.0.join("bad.ts"), "enum Bad { Value }").unwrap();
    let mut services = Services::default();
    let id = services.serve(&root.0).unwrap();
    services.scheme(id, "app://demo/").unwrap();
    for entry in ["main.ts", "main.js"] {
        let url = address(&format!("app://demo/{entry}")).unwrap();
        let response = services.get(&url).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.mime, "text/javascript");
        let text = String::from_utf8(response.body).unwrap();
        assert!(!text.contains("import "));
        assert!(!text.contains(": number"));
        assert!(text.contains("value         = 7"));
    }
    let response = services
        .get(&address("app://demo/bad.ts").unwrap())
        .unwrap();
    assert_eq!(response.status, 400);
    let text = String::from_utf8(response.body).unwrap();
    assert!(text.contains("bad.ts:"));
}
