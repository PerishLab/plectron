use shared::gate::{Fault, Gate};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn generations() {
    let mut gate = Gate::default();
    let first = gate.hold().unwrap();
    assert!(matches!(gate.hold(), Err(Fault::Held)));
    gate.release().unwrap();
    let second = gate.hold().unwrap();
    first.wait().unwrap();
    gate.close();
    assert_eq!(second.wait(), Err(Fault::Cancelled));
    assert_eq!(gate.release(), Err(Fault::Closed));
    assert!(matches!(gate.hold(), Err(Fault::Closed)));
}
#[test]
fn cancellation() {
    let mut gate = Gate::default();
    let ticket = gate.hold().unwrap();
    let (sender, receiver) = mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || sender.send(ticket.wait()).unwrap());
    drop(gate);
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(Fault::Cancelled)
    );
    thread.join().unwrap();
}
#[test]
fn abandoned() {
    let mut gate = Gate::default();
    drop(gate.hold().unwrap());
    gate.release().unwrap();
    let ticket = gate.hold().unwrap();
    gate.release().unwrap();
    gate.close();
    ticket.wait().unwrap();
}

#[test]
fn routing() {
    use shared::route::{self, Route};
    assert_eq!(
        route::select(true, true, || -> Option<()> {
            panic!("unused declaration")
        }),
        Route::Reuse
    );
    assert_eq!(
        route::select(false, false, || -> Option<()> {
            panic!("bypass declaration")
        }),
        Route::Build
    );
    assert_eq!(route::select(false, true, || Some(7)), Route::Update(7));
    assert_eq!(route::select(false, true, || None::<()>), Route::Build);
}
