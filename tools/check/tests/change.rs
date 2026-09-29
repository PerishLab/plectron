use plectron_check::{Kind, parse};

#[test]
fn statuses() {
    let changes = parse(
        "A\ta.rs\nM\tb.rs\nT\tc.rs\nD\td.rs\nR100\told.rs\tnew.rs\nC100\tsource.rs\tcopy.rs\n",
    )
    .unwrap();
    assert_eq!(changes[0].kind, Kind::Added);
    assert_eq!(changes[1].kind, Kind::Modified);
    assert_eq!(changes[2].kind, Kind::Modified);
    assert_eq!(changes[3].kind, Kind::Deleted);
    assert_eq!(changes[4].kind, Kind::Renamed);
    assert_eq!(changes[5].kind, Kind::Copied);
    assert_eq!(
        changes[4].previous.as_deref().unwrap().to_str(),
        Some("old.rs")
    );
    assert_eq!(changes[5].path.to_str(), Some("copy.rs"));
}
