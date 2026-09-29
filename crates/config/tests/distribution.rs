use config::{Command, Store, Value};

#[test]
fn distribution() {
    let store = Store::default();
    let consumer = store.clone();
    store.set("arbitrary", Value::Null);
    let copy = store.inspect();
    std::thread::spawn(move || consumer.set("arbitrary", (-1).into()))
        .join()
        .unwrap();
    assert_eq!(copy["arbitrary"], Value::Null);
    assert_eq!(store.get("arbitrary"), Some((-1).into()));
    assert!(Store::default().get("arbitrary").is_none());
    let entries = store.command(Command::Remove {
        key: "arbitrary".into(),
    });
    assert!(entries.is_empty());
    assert!(store.get("arbitrary").is_none());
}
