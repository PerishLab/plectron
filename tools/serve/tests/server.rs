#![cfg(unix)]

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Serving {
    child: Child,
    root: PathBuf,
    port: u16,
    _directory: tempfile::TempDir,
}

impl Serving {
    fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("assets");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("index.html"), "<p>cards</p>").unwrap();
        fs::write(root.join("style.css"), "body { color: blue; }").unwrap();
        fs::write(root.join("font.ttf"), b"\0font\xff").unwrap();
        fs::write(root.join("hello world.js"), "let label = '中文';").unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_plectron-serve"))
            .args(["--root", root.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let ready: serde_json::Value = serde_json::from_str(
            BufReader::new(child.stdout.take().unwrap())
                .lines()
                .next()
                .expect("server did not announce readiness")
                .unwrap()
                .as_str(),
        )
        .unwrap();
        assert_eq!(
            ready["root"],
            root.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(ready["pid"], child.id());
        let port = ready["url"]
            .as_str()
            .unwrap()
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        Self {
            child,
            root,
            port,
            _directory: directory,
        }
    }

    fn request(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> Reply {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: localhost\r\n").unwrap();
        for (name, value) in headers {
            write!(stream, "{name}: {value}\r\n").unwrap();
        }
        write!(stream, "Connection: close\r\n\r\n").unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        Reply::parse(bytes)
    }

    fn stop(&mut self) {
        if self.child.try_wait().unwrap().is_some() {
            return;
        }
        kill(Pid::from_raw(self.child.id() as i32), Signal::SIGTERM).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "server exited with {status}");
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.child.kill().unwrap();
        panic!("server did not stop");
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn parse(bytes: Vec<u8>) -> Self {
        let split = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap();
        let head = String::from_utf8(bytes[..split].to_vec()).unwrap();
        let mut lines = head.lines();
        let status = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':').unwrap();
                (name.to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect();
        Self {
            status,
            headers,
            body: bytes[split + 4..].to_vec(),
        }
    }

    fn header(&self, name: &str) -> &str {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .unwrap()
            .1
            .as_str()
    }
}

#[test]
fn resources() {
    let server = Serving::start();
    for (path, mime, body) in [
        ("/", "text/html; charset=utf-8", b"<p>cards</p>".as_slice()),
        (
            "/style.css?v=1",
            "text/css; charset=utf-8",
            b"body { color: blue; }".as_slice(),
        ),
        ("/font.ttf", "font/ttf", b"\0font\xff".as_slice()),
        (
            "/hello%20world.js",
            "text/javascript; charset=utf-8",
            "let label = '中文';".as_bytes(),
        ),
    ] {
        let reply = server.request("GET", path, &[]);
        assert_eq!((reply.status, reply.body.as_slice()), (200, body));
        assert_eq!(reply.header("content-type"), mime);
        assert_eq!(reply.header("cache-control"), "no-store");
        assert_eq!(reply.header("content-length"), body.len().to_string());
    }
    let reply = server.request("HEAD", "/font.ttf", &[]);
    assert_eq!((reply.status, reply.body.as_slice()), (200, b"".as_slice()));
    assert_eq!(reply.header("content-length"), "6");
    let modified = reply.header("last-modified").to_owned();
    fs::write(server.root.join("font.ttf"), b"changed").unwrap();
    let reply = server.request("GET", "/font.ttf", &[("If-Modified-Since", &modified)]);
    assert_eq!(
        (reply.status, reply.body.as_slice()),
        (200, b"changed".as_slice())
    );
    assert_eq!(server.request("GET", "/missing", &[]).status, 404);
}

#[test]
fn scope() {
    let server = Serving::start();
    let outside = server.root.parent().unwrap().join("private.txt");
    fs::write(&outside, "outside").unwrap();
    symlink(&outside, server.root.join("escape.txt"));
    fs::create_dir(server.root.join("nested")).unwrap();
    symlink(&outside, server.root.join("nested/index.html"));
    fs::create_dir(server.root.join("empty")).unwrap();
    for path in ["/escape.txt", "/nested/"] {
        assert_eq!(server.request("GET", path, &[]).status, 403);
    }
    assert_eq!(server.request("GET", "/empty/", &[]).status, 404);
    let reply = server.request("GET", "/%2e%2e/private.txt", &[]);
    assert_eq!(reply.status, 404);
    assert!(!reply.body.windows(7).any(|window| window == b"outside"));
}

#[test]
fn lifecycle() {
    let mut server = Serving::start();
    let port = server.port;
    server.stop();
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    for arguments in [
        vec![
            "--root".to_owned(),
            server.root.join("missing").display().to_string(),
        ],
        vec!["--port".to_owned(), "65536".to_owned()],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_plectron-serve"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

fn symlink(original: &Path, link: PathBuf) {
    std::os::unix::fs::symlink(original, link).unwrap();
}
