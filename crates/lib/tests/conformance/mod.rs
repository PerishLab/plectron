mod device;
mod ipc;
mod lifecycle;
mod rendering;
mod resource;
mod runtime;
mod web;

mod applications;

mod presentation;

fn application(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    pt::project::script(&root.join(path)).unwrap().javascript
}
