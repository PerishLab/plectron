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
    let root = fixtures::root().parent().unwrap().to_owned();
    pt::project::script(&root.join(path)).unwrap().javascript
}
