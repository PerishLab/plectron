use super::{error, http};
use crate::contract::Fault;
use percent_encoding::percent_decode_str;
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use webengine::resource::Response;

pub struct Server {
    address: SocketAddr,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(root: &Path) -> Result<Self, Fault> {
        let root = root.canonicalize().map_err(error)?;
        if !root.is_dir() {
            return Err(Fault("static root must be a directory".into()));
        }
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(error)?;
        let address = listener.local_addr().map_err(error)?;
        listener.set_nonblocking(true).map_err(error)?;
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let worker = thread::Builder::new()
            .name("static".into())
            .spawn(move || {
                serve(listener, &root, stop);
            })
            .map_err(error)?;
        eprintln!("[static] http://{address}");
        Ok(Self {
            address,
            stopping,
            worker: Some(worker),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve(listener: TcpListener, root: &Path, stop: Arc<AtomicBool>) {
    let root = Arc::new(root.to_path_buf());
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                let _ = workers.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let root = Arc::clone(&root);
                workers.push(thread::spawn(move || respond(stream, &root)));
            }
            Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5))
            }
            Err(_) => break,
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
}

fn respond(mut stream: TcpStream, root: &Path) {
    let response = handle(&mut stream, root).unwrap_or_else(|fault| Response {
        status: 400,
        mime: "text/plain".into(),
        body: fault.to_string().into_bytes(),
    });
    let _ = http::send(&mut stream, response);
}

fn handle(stream: &mut TcpStream, root: &Path) -> Result<Response, Fault> {
    stream.set_nonblocking(false).map_err(error)?;
    let head = http::head(stream)?;
    let request: Vec<_> = head
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    let [method, path, "HTTP/1.1"] = request.as_slice() else {
        return Err(Fault("invalid HTTP request".into()));
    };
    if *method != "GET" {
        return Ok(status(405));
    }
    let path = percent_decode_str(path).decode_utf8().map_err(error)?;
    let relative = path
        .strip_prefix('/')
        .ok_or_else(|| Fault("expected absolute request path".into()))?;
    if relative.contains(['\\', '\0', '?', '#']) {
        return Ok(status(403));
    }
    if Path::new(relative)
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Ok(status(403));
    }
    let target = match root.join(relative).canonicalize() {
        Ok(path) => path,
        Err(_) => return Ok(status(404)),
    };
    if !target.starts_with(root) || !target.is_file() {
        return Ok(status(403));
    }
    let extension = target.extension().and_then(|part| part.to_str());
    let module = matches!(extension, Some("js" | "pt"));
    if module {
        let mut workspace = project::Workspace::open(
            &target,
            project::Adapters::new(project::Unavailable, pt::Templates),
        )
        .map_err(|failure| Fault(failure.0))?;
        let javascript = workspace
            .link(project::Target::Script)
            .map_err(|failure| Fault(failure.to_string()))?
            .javascript;
        if javascript.len() > webengine::resource::Kind::Text.limit() {
            return Ok(status(413));
        }
        return Ok(Response {
            status: 200,
            mime: "text/javascript".into(),
            body: javascript.into_bytes(),
        });
    }
    let limit = match target.extension().and_then(|s| s.to_str()) {
        Some("ttf") => webengine::resource::Kind::Font.limit(),
        Some("png") => webengine::resource::Kind::Image.limit(),
        _ => webengine::resource::Kind::Text.limit(),
    };
    let mut body = Vec::new();
    std::fs::File::open(&target)
        .map_err(error)?
        .take(limit as u64 + 1)
        .read_to_end(&mut body)
        .map_err(error)?;
    if body.len() > limit {
        return Ok(status(413));
    }
    let mime = match target.extension().and_then(|s| s.to_str()) {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "text/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    };
    Ok(Response {
        status: 200,
        mime: mime.into(),
        body,
    })
}

fn status(status: u16) -> Response {
    Response {
        status,
        mime: "text/plain".into(),
        body: format!("resource status {status}").into_bytes(),
    }
}
