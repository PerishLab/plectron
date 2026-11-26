use graphics::shared::{Backend, Cpu};
use shell::{Bundle, Memory, Options};
use std::process::ExitCode;
use std::sync::Arc;
use webengine::{Fault, Page};

const HTML: &str = r#"<!doctype html><html><head><title>Plectron Shell</title><link rel="stylesheet" href="app.css"><script src="app.js"></script></head><body><div class="card"><h1>Plectron Standalone</h1><p id="status">Loading...</p></div></body></html>"#;
const CSS: &str = r#"body { margin: 0; padding: 32px; background-color: #f4f6f9; color: #16212e; font-family: "Segoe UI", "Microsoft YaHei", sans-serif; } .card { padding: 24px; background-color: #ffffff; } h1 { font-size: 28px; margin: 0; } #status { font-size: 16px; margin: 12px; color: #36506b; }"#;
const SCRIPT: &str = r#"Symbol[Symbol.plectron].document.getElementById("status").setText("Ready · 独立运行时已就绪");"#;

fn main() -> ExitCode {
    match start() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("plectron-shell: {error}");
            ExitCode::FAILURE
        }
    }
}

fn start() -> Result<(), Fault> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [flag] if flag == "--probe" => probe(builtin()?),
        [flag, path] if flag == "--probe" => probe(load(path)?),
        [path] => shell::run(load(path)?, Options::new("Plectron", 720, 480)),
        [] => shell::run(builtin()?, Options::new("Plectron Shell", 720, 480)),
        _ => Err(Fault(
            "usage: plectron-shell [--probe] [bundle.pbnd]".into(),
        )),
    }
}

fn load(path: &std::ffi::OsStr) -> Result<Bundle, Fault> {
    let bytes = std::fs::read(path).map_err(|error| Fault(error.to_string()))?;
    Bundle::decode(&bytes)
}

fn builtin() -> Result<Bundle, Fault> {
    let mut bundle = Bundle::new("app://shell/", "app://shell/index.html")?;
    bundle.insert("/index.html", "text/html", HTML.as_bytes().to_vec())?;
    bundle.insert("/app.css", "text/css", CSS.as_bytes().to_vec())?;
    bundle.insert("/app.js", "text/javascript", SCRIPT.as_bytes().to_vec())?;
    Ok(bundle)
}

fn probe(bundle: Bundle) -> Result<(), Fault> {
    let mut provider = Memory::new(Arc::new(bundle)).system();
    let mut page = Page::new(
        provider.entry(),
        domengine::Extent {
            width: 720,
            height: 480,
        },
    )?;
    page.load(&mut provider)?;
    let frame = page
        .frame()
        .ok_or_else(|| Fault("standalone page produced no frame".into()))?;
    let image = Cpu
        .render(&frame)
        .map_err(|error| Fault(error.to_string()))?;
    println!("ok {}x{}", image.extent.width, image.extent.height);
    Ok(())
}
