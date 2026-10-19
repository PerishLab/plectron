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
    desktop.windows.get_mut(&unit).unwrap().timing = timing::Timing::timer();
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
        desktop.windows[&unit].timing.native(),
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
    let mut leads = Vec::new();
    let started = std::time::Instant::now();
    while pulses.len() < 31 && started.elapsed() < std::time::Duration::from_secs(5) {
        desktop.wait(Pacing::Continuous).unwrap();
        desktop.poll().unwrap();
        if let Some(pulse) = desktop.pulse(unit).unwrap() {
            let now = std::time::Instant::now();
            leads.push(pulse.target().saturating_duration_since(now).as_secs_f64() * 1000.0);
            pulses.push(now.max(pulse.target()));
        }
    }
    assert!(pulses.len() >= 2, "continuous pacing produced no pulses");
    let mut intervals: Vec<f64> = pulses
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
        .collect();
    intervals.sort_by(f64::total_cmp);
    leads.sort_by(f64::total_cmp);
    let median = intervals[intervals.len() / 2];
    eprintln!(
        "linux::tests::cadence source={} pulses={} median_ms={median:.3} min_ms={:.3} max_ms={:.3} lead_ms={:.3}",
        if desktop.windows[&unit].timing.native() {
            "present-msc"
        } else {
            "timer-16ms"
        },
        pulses.len(),
        intervals[0],
        intervals[intervals.len() - 1],
        leads[leads.len() / 2],
    );
    assert!(median >= 2.0, "continuous pacing spun without a cadence");
}

#[test]
fn pair() {
    let mut desktop = match Desktop::new("cpu", |fault| fault) {
        Ok(desktop) => desktop,
        Err(fault) => {
            eprintln!("skip linux::tests::pair: no reachable X11 display ({fault})");
            return;
        }
    };
    let units = [Unit::new(1), Unit::new(2)];
    for (index, unit) in units.iter().enumerate() {
        let options = Options {
            title: format!("Plectron pacing probe {index}"),
            extent: Extent {
                width: 64,
                height: 32,
            },
        };
        desktop.create(*unit, &options).unwrap();
    }
    let sources: Vec<_> = units
        .iter()
        .map(|unit| desktop.windows[unit].timing.source())
        .collect();
    if let [Some((first, window)), Some((second, other))] = sources[..] {
        assert_eq!(window, desktop.windows[&units[0]].xid);
        assert_eq!(other, desktop.windows[&units[1]].xid);
        assert_ne!(first, second);
        for unit in units {
            desktop.pace(unit, Pacing::Continuous).unwrap();
        }
        let mut event = x11rb::protocol::present::CompleteNotifyEvent {
            response_type: 35,
            extension: 0,
            sequence: 0,
            length: 0,
            event_type: 1,
            kind: x11rb::protocol::present::CompleteKind::NOTIFY_MSC,
            mode: x11rb::protocol::present::CompleteMode::COPY,
            event: first,
            window: other,
            serial: 0,
            ust: 0,
            msc: 1,
        };
        assert!(
            !desktop.complete(&event),
            "foreign context reached a window"
        );
        event.window = 0;
        assert!(!desktop.complete(&event), "unknown window was routed");
        event.window = window;
        assert!(desktop.complete(&event), "own completion was not routed");
        assert!(desktop.pulse(units[0]).unwrap().is_some());
        assert_eq!(desktop.demand[&units[0]].seen, 1);
        assert!(desktop.pulse(units[1]).unwrap().is_none());
        for unit in units {
            desktop.pace(unit, Pacing::Idle).unwrap();
        }
    }
    desktop.pace(units[0], Pacing::Continuous).unwrap();
    desktop.pace(units[1], Pacing::Idle).unwrap();
    let counts = count(&mut desktop, units);
    assert!(counts[0] >= 2, "continuous window produced no pulses");
    assert_eq!(counts[1], 0, "idle window pulsed");
    desktop.pace(units[1], Pacing::Continuous).unwrap();
    let counts = count(&mut desktop, units);
    assert!(
        counts.iter().all(|count| *count >= 2),
        "pair cadence {counts:?}"
    );
    eprintln!(
        "linux::tests::pair native={} pulses={counts:?}",
        desktop.windows[&units[0]].timing.native()
    );
    desktop.windows.get_mut(&units[1]).unwrap().timing = timing::Timing::timer();
    let counts = count(&mut desktop, units);
    assert!(
        counts.iter().all(|count| *count >= 2),
        "mixed cadence {counts:?}"
    );
    eprintln!("linux::tests::pair mixed pulses={counts:?}");
    for unit in units {
        desktop.close(unit).unwrap();
    }
}

fn count(desktop: &mut Desktop<Error>, units: [Unit; 2]) -> [usize; 2] {
    let mut counts = [0, 0];
    let started = std::time::Instant::now();
    while started.elapsed() < std::time::Duration::from_millis(300) {
        desktop.wait(Pacing::Continuous).unwrap();
        desktop.poll().unwrap();
        for (unit, count) in units.iter().zip(&mut counts) {
            if desktop.pulse(*unit).unwrap().is_some() {
                *count += 1;
            }
        }
    }
    counts
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
