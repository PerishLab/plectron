#![cfg(test)]

use super::*;

#[test]
fn projection() {
    let clock = Instant::now();
    assert_eq!(
        instant(clock, 10.0, 10.25),
        Some(clock + Duration::from_millis(250))
    );
}

#[test]
fn rejection() {
    let clock = Instant::now();
    assert_eq!(instant(clock, 10.0, 9.0), None);
    assert_eq!(instant(clock, 10.0, f64::NAN), None);
    assert_eq!(instant(clock, 10.0, f64::INFINITY), None);
}
