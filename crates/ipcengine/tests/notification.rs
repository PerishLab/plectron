use ipcengine::message;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn delivery() {
    let owner = shared::Notify::thread(std::thread::current());
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || message::ask(&sender, 21, Some(&owner)).unwrap());
    std::thread::park_timeout(Duration::from_secs(1));
    let request = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!request.expired());
    request.reply.send(request.request * 2).unwrap();
    assert_eq!(worker.join().unwrap(), 42);
}

#[test]
fn disconnected() {
    let (sender, receiver) = mpsc::sync_channel::<message::Envelope<(), ()>>(1);
    drop(receiver);
    let notify = shared::Notify::thread(std::thread::current());
    assert!(message::ask(&sender, (), Some(&notify)).is_err());
}

#[test]
fn pressure() {
    let (sender, _receiver) = mpsc::sync_channel::<message::Envelope<(), ()>>(0);
    assert!(message::ask(&sender, (), None).is_err());
}
