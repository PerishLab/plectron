use graphics::shared::Pacing;
use std::time::{Duration, Instant};

#[test]
fn priority() {
    let now = Instant::now();
    let later = now + Duration::from_secs(1);
    assert_eq!(Pacing::Idle.combine(Pacing::Idle), Pacing::Idle);
    assert_eq!(Pacing::Idle.combine(Pacing::At(later)), Pacing::At(later));
    assert_eq!(
        Pacing::At(later).combine(Pacing::Continuous),
        Pacing::Continuous
    );
}

#[test]
fn earliest() {
    let first = Instant::now();
    let second = first + Duration::from_secs(1);
    assert_eq!(
        Pacing::At(second).combine(Pacing::At(first)),
        Pacing::At(first)
    );
    assert_eq!(Pacing::At(first).deadline(), Some(first));
    assert_eq!(Pacing::Continuous.deadline(), None);
}
