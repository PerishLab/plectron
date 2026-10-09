use clap::Parser;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

#[derive(Parser)]
#[command(
    name = "plectron-serve",
    about = "Serve local static assets on loopback HTTP.",
    version = plumb::version!("PLECTRON_SERVE")
)]
struct Cli {
    #[arg(long, default_value_os_t = default_root())]
    root: PathBuf,
    #[arg(long, default_value_t = 0, help = "listen port; 0 selects a free port")]
    port: u16,
}

#[derive(Serialize)]
struct Ready<'a> {
    url: String,
    root: &'a Path,
    pid: u32,
}

enum Resource {
    File(PathBuf),
    Forbidden,
    Missing,
}

fn main() -> ExitCode {
    if let Err(error) = plumb::identity!("PLECTRON_SERVE") {
        eprintln!("serve: {error}");
        return ExitCode::from(1);
    }
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("serve: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let root = cli
        .root
        .canonicalize()
        .map_err(|_| "root must be an existing directory".to_owned())?;
    if !root.is_dir() {
        return Err("root must be an existing directory".to_owned());
    }
    let server = Server::http(("127.0.0.1", cli.port)).map_err(|error| error.to_string())?;
    let port = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| "loopback listener has no IP address".to_owned())?
        .port();
    println!(
        "{}",
        serde_json::to_string(&Ready {
            url: format!("http://127.0.0.1:{port}/"),
            root: &root,
            pid: std::process::id(),
        })
        .map_err(|error| error.to_string())?
    );

    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::Release))
        .map_err(|error| error.to_string())?;
    while running.load(Ordering::Acquire) {
        if let Some(request) = server
            .recv_timeout(Duration::from_millis(50))
            .map_err(|error| error.to_string())?
        {
            let root = root.clone();
            std::thread::spawn(move || respond(request, &root));
        }
    }
    Ok(())
}

fn default_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps")
}

fn respond(request: Request, root: &Path) {
    if !matches!(request.method(), Method::Get | Method::Head) {
        send_error(request, StatusCode(405), "Method not allowed");
        return;
    }
    match locate(root, request.url()) {
        Resource::File(path) => send_file(request, &path),
        Resource::Forbidden => send_error(
            request,
            StatusCode(403),
            "Resource is outside the static root",
        ),
        Resource::Missing => send_error(request, StatusCode(404), "Not found"),
    }
}

fn locate(root: &Path, url: &str) -> Resource {
    let encoded = url.split('?').next().unwrap_or(url);
    let Ok(decoded) = percent_decode_str(encoded).decode_utf8() else {
        return Resource::Missing;
    };
    let mut target = root.to_owned();
    for part in decoded.split('/') {
        if part.is_empty() || matches!(part, "." | "..") {
            continue;
        }
        target.push(part);
    }
    let Ok(resolved) = target.canonicalize() else {
        return Resource::Missing;
    };
    if !resolved.starts_with(root) {
        return Resource::Forbidden;
    }
    if resolved.is_file() {
        return Resource::File(resolved);
    }
    if !resolved.is_dir() {
        return Resource::Missing;
    }
    for name in ["index.html", "index.htm"] {
        let candidate = resolved.join(name);
        if !candidate.exists() {
            continue;
        }
        let Ok(candidate) = candidate.canonicalize() else {
            return Resource::Forbidden;
        };
        if !candidate.starts_with(root) {
            return Resource::Forbidden;
        }
        if candidate.is_file() {
            return Resource::File(candidate);
        }
    }
    Resource::Missing
}

fn send_file(request: Request, path: &Path) {
    let Ok(metadata) = path.metadata() else {
        send_error(request, StatusCode(404), "Not found");
        return;
    };
    let length = metadata.len();
    if request.method() == &Method::Head {
        let response = resource_headers(
            Response::empty(StatusCode(200))
                .with_header(header("Content-Length", &length.to_string())),
            path,
            &metadata,
        );
        let _ = request.respond(response);
        return;
    }
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            send_error(request, StatusCode(404), "Not found");
            return;
        }
        Err(_) => {
            send_error(request, StatusCode(500), "Resource could not be read");
            return;
        }
    };
    let response = resource_headers(
        Response::from_data(data).with_status_code(StatusCode(200)),
        path,
        &metadata,
    );
    let _ = request.respond(response);
}

fn resource_headers<R: Read>(
    mut response: Response<R>,
    path: &Path,
    metadata: &fs::Metadata,
) -> Response<R> {
    response.add_header(header("Content-Type", content_type(path)));
    response.add_header(header("Cache-Control", "no-store"));
    if let Ok(modified) = metadata.modified() {
        response.add_header(header("Last-Modified", &httpdate::fmt_http_date(modified)));
    }
    response
}

fn send_error(request: Request, status: StatusCode, message: &str) {
    let response = Response::from_string(message)
        .with_status_code(status)
        .with_header(header("Content-Type", "text/plain; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"));
    let _ = request.respond(response);
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).expect("static header name and valid value")
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}
