use super::{Policy, workload::Workload};
use crate::Result;
use fixtures::Text;
use serde::Serialize;
use shared::cache::{Command, Manager};

mod summary;

#[derive(Serialize)]
pub struct Diagnosis {
    schema: u32,
    scenario: String,
    policy: Policy,
    summary: summary::Summary,
    causes: Vec<summary::Cause>,
    stages: Vec<Stage>,
}

#[derive(Serialize)]
struct Stage {
    index: usize,
    dropped: usize,
    operations: domengine::Operations,
    charged: usize,
    entries: usize,
    bytes: usize,
    windowed: usize,
    windowpeak: usize,
    windowbytes: usize,
    summary: summary::Summary,
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<Vec<domengine::Event>>,
}

pub fn run(scenario: &str, policy: Policy, events: bool) -> Result<Diagnosis> {
    let mut workload = Workload::new(scenario)?;
    let manager = Manager::default();
    let config = config::Store::default();
    if let Some(pages) = policy.pages {
        config.set("layout.pages", pages.into());
    }
    if let Some(admission) = policy.admission {
        config.set("layout.admission", admission.into());
    }
    if let Some(recurrence) = policy.recurrence {
        config.set("layout.recurrence", recurrence.into());
    }
    if let Some(window) = policy.window {
        config.set("layout.window", window.into());
    }
    let mut pipeline = domengine::Pipeline::configured(Text, &manager, config)
        .map_err(|error| error.to_string())?;
    for cache in manager.inspect() {
        let mut target = cache.target;
        target.read = true;
        target.write = true;
        manager
            .command(cache.id, Command::Configure { policy: target })
            .map_err(|error| error.to_string())?;
    }
    let resolver = domengine::style::Resolver::managed(domengine::Sheet::default(), &manager)
        .map_err(|error| error.to_string())?;
    let mut builder = domengine::Capture::new(resolver);
    builder.enable(true);
    for index in 0..4 {
        workload.update(index)?;
        builder
            .update(&workload.document, |input| {
                pipeline.build(input, workload.constraints())
            })
            .map_err(|error| error.to_string())?;
    }
    let mut causality = domengine::Causality::new(16_384);
    let mut records = Vec::new();
    let mut stages = Vec::new();
    for index in 4..12 {
        causality.clear();
        workload.update(index)?;
        builder
            .update(&workload.document, |input| {
                pipeline.diagnose(input, workload.constraints(), &mut causality)
            })
            .map_err(|error| error.to_string())?;
        let report = pipeline.inspect();
        let (summary, _) = summary::compile(causality.events());
        records.push(causality.events().to_vec());
        stages.push(Stage {
            index,
            dropped: causality.dropped(),
            operations: report.operations,
            charged: report.charged,
            entries: report.entries,
            bytes: report.bytes,
            windowed: report.windowed,
            windowpeak: report.windowpeak,
            windowbytes: report.windowbytes,
            summary,
            events: events.then(|| causality.events().to_vec()),
        });
    }
    let (summary, causes) = summary::aggregate(&records);
    Ok(Diagnosis {
        schema: 1,
        scenario: scenario.into(),
        policy,
        summary,
        causes,
        stages,
    })
}
