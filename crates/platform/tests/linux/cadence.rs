#![cfg(test)]

use super::*;
use crate::shared::Extent;

#[test]
fn cadence() {
    let mut desktop = match Desktop::new("cpu", |fault| fault) {
        Ok(desktop) => desktop,
        Err(fault) => {
            eprintln!("skip linux::tests::cadence: no reachable X11 display ({fault})");
            return;
        }
    };
    let unit = Unit::new(1);
    let options = Options {
        title: "Plectron cadence probe".into(),
        extent: Extent {
            width: 64,
            height: 32,
        },
    };
    desktop.create(unit, &options).unwrap();
    desktop.pace(unit, Pacing::Continuous).unwrap();
    measure(&mut desktop, unit);
    deadline(&mut desktop, unit);
    desktop.timing = timing::Timing::timer();
    desktop.pace(unit, Pacing::Continuous).unwrap();
    measure(&mut desktop, unit);
    deadline(&mut desktop, unit);
    desktop.close(unit).unwrap();
    assert!(desktop.pace(unit, Pacing::Idle).is_err());
}

fn deadline(desktop: &mut Desktop<Error>, unit: Unit) {
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_millis(30);
    desktop.pace(unit, Pacing::At(deadline)).unwrap();
    let mut pulse = None;
    while pulse.is_none() && started.elapsed() < std::time::Duration::from_secs(2) {
        desktop.wait(Pacing::At(deadline)).unwrap();
        desktop.poll().unwrap();
        pulse = desktop.pulse(unit).unwrap();
    }
    assert!(pulse.is_some(), "deadline pacing produced no pulse");
    assert!(std::time::Instant::now() >= deadline);
    eprintln!(
        "linux::tests::cadence deadline native={} late_ms={:.3}",
        desktop.timing.native(),
        (std::time::Instant::now() - deadline).as_secs_f64() * 1000.0
    );
    desktop.pace(unit, Pacing::Idle).unwrap();
    let quiet = std::time::Instant::now() + std::time::Duration::from_millis(50);
    while std::time::Instant::now() < quiet {
        desktop.wait(Pacing::At(quiet)).unwrap();
        desktop.poll().unwrap();
        assert!(desktop.pulse(unit).unwrap().is_none());
    }
}

fn measure(desktop: &mut Desktop<Error>, unit: Unit) {
    let mut pulses = Vec::new();
    let started = std::time::Instant::now();
    while pulses.len() < 31 && started.elapsed() < std::time::Duration::from_secs(5) {
        desktop.wait(Pacing::Continuous).unwrap();
        desktop.poll().unwrap();
        if let Some(pulse) = desktop.pulse(unit).unwrap() {
            pulses.push(std::time::Instant::now().max(pulse.target()));
        }
    }
    assert!(pulses.len() >= 2, "continuous pacing produced no pulses");
    let mut intervals: Vec<f64> = pulses
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
        .collect();
    intervals.sort_by(f64::total_cmp);
    let median = intervals[intervals.len() / 2];
    eprintln!(
        "linux::tests::cadence source={} pulses={} median_ms={median:.3} min_ms={:.3} max_ms={:.3}",
        if desktop.timing.native() {
            "present-msc"
        } else {
            "timer-16ms"
        },
        pulses.len(),
        intervals[0],
        intervals[intervals.len() - 1],
    );
    assert!(median >= 2.0, "continuous pacing spun without a cadence");
}

#[test]
fn refusal() {
    assert!(Desktop::<Error>::new("metal", |fault| fault).is_err());
    let mut desktop = match Desktop::new("cpu", |fault| fault) {
        Ok(desktop) => desktop,
        Err(_) => {
            eprintln!("skip linux::tests::refusal ownership: no reachable X11 display");
            return;
        }
    };
    let unit = Unit::new(1);
    let mut options = Options {
        title: "invalid\0title".into(),
        extent: Extent {
            width: 64,
            height: 32,
        },
    };
    assert!(desktop.create(unit, &options).is_err());
    options.title = "Plectron ownership probe".into();
    desktop.create(unit, &options).unwrap();
    assert!(desktop.create(unit, &options).is_err());
    desktop.focus(unit, None).unwrap();
    desktop.close(unit).unwrap();
    assert!(desktop.focus(unit, None).is_err());
    assert!(desktop.title(unit, "closed").is_err());
    assert!(desktop.close(unit).is_err());
}
