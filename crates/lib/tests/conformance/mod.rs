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
    let mut workspace = project::Workspace::open(root.join(path), pt::Templates).unwrap();
    workspace.link(project::Target::Script).unwrap().javascript
}
