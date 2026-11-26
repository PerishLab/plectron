use graphics::shared::{Backend, Cpu};
use plectron::resource::Services;
use shell::{Bundle, Memory};
use std::sync::Arc;
use webengine::Page;

fn render(origin: &str, entry: &str, size: (usize, usize)) {
    let root = fixtures::root();
    let compiled = plectron_cli::bundle::compile(&root, origin, entry).unwrap();
    let first = compiled.encode().unwrap();
    let second = compiled.encode().unwrap();
    assert_eq!(first, second, "bundle encoding must be deterministic");
    let decoded = Bundle::decode(&first).unwrap();
    assert_eq!(decoded, compiled);

    let extent = domengine::Extent {
        width: size.0,
        height: size.1,
    };
    let mut memory = Memory::new(Arc::new(decoded));
    let mut bundled = Page::new(memory.entry(), extent).unwrap();
    bundled.load(&mut memory).unwrap();
    for _ in 0..16 {
        if !bundled.poll().unwrap() {
            break;
        }
    }
    bundled.render().unwrap();

    let mut services = Services::default();
    let id = services.serve(&root).unwrap();
    services.scheme(id, origin).unwrap();
    let mut dev = Page::new(memory.entry(), extent).unwrap();
    dev.load(&mut services).unwrap();
    for _ in 0..16 {
        if !dev.poll().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    dev.render().unwrap();

    assert_eq!(bundled.state(), webengine::State::Ready);
    assert_eq!(dev.state(), webengine::State::Ready);
    let left = Cpu
        .render(&bundled.frame().expect("bundled frame"))
        .unwrap();
    let right = Cpu.render(&dev.frame().expect("dev frame")).unwrap();
    assert_eq!(left.extent, right.extent);
    assert_eq!(left.pixels, right.pixels);
}

#[test]
fn cards() {
    render("app://cards/", "cards/src/index.html", (720, 640));
}

#[test]
fn chat() {
    render("app://chat/", "chat/src/index.html", (1280, 800));
}
