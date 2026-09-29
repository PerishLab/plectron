use super::{Comparison, Inspection, Manifest, Summary, Timing};
use std::fmt::Write;

impl Inspection {
    pub fn text(&self) -> String {
        let mut out = format!("inspection: {}\n", status(self.valid()));
        issues(&mut out, &self.issues);
        for (name, summary) in &self.reports {
            let _ = writeln!(out, "\n{name}");
            summary.append(&mut out);
        }
        out
    }
}
impl Summary {
    fn append(&self, out: &mut String) {
        let _ = writeln!(
            out,
            "{}: {} | runs {}/{} | milliseconds",
            self.case.as_deref().unwrap_or("?"),
            status(self.valid()),
            self.runs,
            self.expected.map_or("?".into(), |v| v.to_string())
        );
        issues(out, &self.issues);
        let startup = |key: &str| self.startup.get(key).and_then(Option::as_ref);
        let _ = writeln!(
            out,
            "startup load={} resources={} fonts={} fontrequest={} fontparse={} activation={} scene={} publication={}",
            median(startup("load")),
            median(startup("resources")),
            median(startup("fonts")),
            median(startup("fontrequest")),
            median(startup("fontparse")),
            median(startup("activation")),
            median(startup("scene")),
            median(startup("publication")),
        );
        let _ = writeln!(
            out,
            "action       frames   median      p95      cpu observed      pub    paint     prep complete maxΔ"
        );
        for (name, action) in &self.actions {
            let timing = |key: &str| action.timing.get(key).and_then(Option::as_ref);
            let _ = writeln!(
                out,
                "{name:<12} {:>3}/{:<3} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>4}",
                action.frames,
                action.samples,
                median(timing("total")),
                number(timing("total").map(|v| v.p95)),
                median(timing("cpu")),
                median(timing("observed")),
                median(timing("publication")),
                median(timing("paint")),
                median(timing("preparation")),
                median(timing("completion")),
                channel(action.maximum)
            );
        }
        let _ = writeln!(out, "-- = unavailable; maxΔ = reported channel difference");
        let _ = writeln!(
            out,
            "cpu = evaluation + publication + preparation; observed = total + inspection; sums are computed per sample"
        );
    }
}
impl Comparison {
    pub fn text(&self) -> String {
        let mut out = format!(
            "comparison: {} | compatible: {} | milliseconds\n",
            status(self.passed),
            self.compatible
        );
        issues(&mut out, &self.issues);
        let _ = writeln!(
            out,
            "action       samples   before    after    delta        %  output workΔ maxΔ"
        );
        for (name, action) in &self.actions {
            let total = action.timing.get("total");
            let changed = action
                .work
                .values()
                .filter(|v| v.equal == Some(false))
                .count();
            let _ = writeln!(
                out,
                "{name:<12} {:>3}/{:<3} {:>8} {:>8} {:>8} {:>8} {:>7} {:>5} {}/{}",
                action.samples[0],
                action.samples[1],
                median(total.and_then(|v| v.before.as_ref())),
                median(total.and_then(|v| v.after.as_ref())),
                number(total.and_then(|v| v.absolute)),
                number(total.and_then(|v| v.percent)),
                equality(action.output.equal),
                changed,
                channel(action.maximum[0]),
                channel(action.maximum[1])
            );
        }
        let _ = writeln!(
            out,
            "timing deltas are observations; workΔ counts changed counters; -- = unavailable"
        );
        out
    }
}
impl Manifest {
    pub fn text(&self) -> String {
        let mut out = format!(
            "experiment: {} | {}/{} jobs\n",
            status(self.valid()),
            self.entries.len(),
            self.plan.entries.len()
        );
        for entry in &self.entries {
            let _ = writeln!(
                out,
                "\n{} | report: {}",
                entry.id,
                entry.report.as_deref().unwrap_or("missing")
            );
            if let Some(error) = &entry.error {
                let _ = writeln!(out, "  ! {error}");
            }
            if let Some(summary) = &entry.summary {
                summary.append(&mut out);
            }
            if let Some(comparison) = &entry.comparison {
                out.push_str(&comparison.text());
            }
        }
        out
    }
}
fn status(passed: bool) -> &'static str {
    if passed { "passed" } else { "failed" }
}
fn equality(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "same",
        Some(false) => "changed",
        None => "--",
    }
}
fn issues(out: &mut String, issues: &[String]) {
    for issue in issues {
        let _ = writeln!(out, "  ! {issue}");
    }
}
fn number(value: Option<f64>) -> String {
    value.map_or("--".into(), |v| format!("{v:.3}"))
}
fn median(value: Option<&Timing>) -> String {
    number(value.map(|v| v.median))
}
fn channel(value: Option<u64>) -> String {
    value.map_or("--".into(), |v| v.to_string())
}
