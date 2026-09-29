use graphics::shared::{Compute, Extent, Frame, Output, Scene, State, Visual};
use std::sync::mpsc;

#[test]
fn completion() {
    let (notices, observed) = mpsc::sync_channel(1);
    let notify = shared::Notify::new(move || notices.send(()).unwrap());
    let compute = Compute::new(notify).unwrap();
    let extent = Extent {
        width: 2,
        height: 2,
    };
    let scene = Scene::new(extent, 0, Vec::new()).unwrap();
    let frame = Frame::new(scene, Visual::new(extent)).unwrap();
    let mut task = compute
        .submit(frame.prepare(Output::new(extent, 1.0).unwrap()).unwrap())
        .unwrap();
    observed
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("compute completion notification");
    assert_eq!(task.poll(), State::Complete);
    let image = task.take().unwrap().expect("completed image");
    assert_eq!(image.extent, extent);
    assert_eq!(image.pixels.len(), 4);
}
