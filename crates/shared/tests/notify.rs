use shared::Notify;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn callback() {
    let (sender, receiver) = mpsc::channel();
    let notify = Notify::new(move || sender.send(()).unwrap());
    std::thread::spawn(move || notify.notify()).join().unwrap();
    receiver.recv_timeout(Duration::from_secs(1)).unwrap();
}
