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
    measure(&mut desktop, unit);
    desktop.timing = timing::Timing::timer();
    measure(&mut desktop, unit);
    desktop.close(unit).unwrap();
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
