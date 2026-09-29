use geoengine::{Coverage, Span, Unit};

fn span(start: i32, end: i32) -> Span {
    Span::new(Unit::new(start), Unit::new(end)).unwrap()
}

#[test]
fn exactcoverage() {
    let coverage = Coverage::observe(
        span(100, 300),
        &[span(20, 80), span(80, 180), span(180, 380)],
    )
    .unwrap()
    .unwrap();
    assert_eq!(coverage.leading(), 80);
    assert_eq!(coverage.trailing(), 80);
    assert_eq!(coverage.items(), 3);
    assert!(coverage.saturated(Unit::new(64)));
    assert!(!coverage.saturated(Unit::new(96)));
}

#[test]
fn partialcoverage() {
    let coverage = Coverage::observe(span(100, 300), &[span(140, 220), span(220, 280)])
        .unwrap()
        .unwrap();
    assert_eq!(coverage.leading(), -40);
    assert_eq!(coverage.trailing(), -20);
}

#[test]
fn emptycoverage() {
    assert_eq!(Coverage::observe(span(0, 100), &[]).unwrap(), None);
}

#[test]
fn monotonicflow() {
    assert!(Coverage::observe(span(0, 100), &[span(0, 60), span(40, 80)]).is_err());
    assert!(Span::new(Unit::new(10), Unit::new(5)).is_err());
}

#[test]
fn saturatedwindow() {
    let viewport = span(100, 300);
    let items = [span(0, 80), span(80, 180), span(180, 280), span(280, 380)];
    assert_eq!(
        Coverage::window(viewport, &items, Unit::new(40)).unwrap(),
        0..4
    );
    assert_eq!(
        Coverage::window(viewport, &items, Unit::new(0)).unwrap(),
        1..4
    );
    assert_eq!(
        Coverage::window(span(500, 600), &items, Unit::new(0)).unwrap(),
        0..0
    );
}
