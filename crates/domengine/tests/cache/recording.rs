use domengine::{Capture, Painter, Pipeline, Sheet, html, style::Resolver};
use fixtures::Text;
use shared::cache::Manager;
use std::time::Instant;

const SAMPLES: usize = 30;
const WARMUPS: usize = 3;

fn exercise(count: usize, enabled: bool) -> Vec<serde_json::Value> {
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("paint.routing", enabled.into());
    let mut painter = Painter::configured(&manager, config).unwrap();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let mut capture = Capture::new(Resolver::managed(Sheet::default(), &manager).unwrap());
    let mut document = html::parse(&format!(
        "<div id='body' style='height:64px;overflow:hidden;'>{}</div>",
        "<p>Alpha beta</p>".repeat(count)
    ))
    .unwrap();
    let node = document.find("body").unwrap();
    let mut previous = capture
        .update(&document, |input| {
            pipeline.build(input, super::constraints(128))
        })
        .unwrap();
    painter.build(&previous).unwrap();
    let mut observations = Vec::new();
    for sample in 0..SAMPLES + WARMUPS {
        let opacity = if sample.is_multiple_of(2) { "0.5" } else { "1" };
        document
            .attribute(
                node,
                "style",
                &format!("height:64px;overflow:hidden;opacity:{opacity};"),
            )
            .unwrap();
        let current = capture
            .update(&document, |input| {
                pipeline.build(input, super::constraints(128))
            })
            .unwrap();
        let before = requests(&manager);
        let start = Instant::now();
        let items = painter.build(&current).unwrap();
        let elapsed = start.elapsed().as_nanos();
        let work = painter.inspect();
        assert_eq!(work.routed, usize::from(enabled));
        if sample == WARMUPS {
            let scene =
                graphics::shared::Scene::new(current.extent(), current.revision(), items).unwrap();
            let frame = graphics::shared::Frame::new(
                scene,
                graphics::shared::Visual::new(current.extent()),
            )
            .unwrap();
            assert_eq!(
                graphics::shared::Backend::render(&mut graphics::shared::Cpu, &frame).unwrap(),
                super::pixels(&current)
            );
        }
        if sample >= WARMUPS {
            observations.push(serde_json::json!({"count":count,"enabled":enabled,"sample":sample,"elapsed":elapsed,"lookups":requests(&manager)-before,"work":work}));
        }
        previous = current;
    }
    drop(previous);
    observations
}

#[test]
#[ignore = "timing observation, not a correctness check"]
fn composition() {
    let mut observations = Vec::new();
    for count in [64, 512] {
        for enabled in [false, true] {
            observations.extend(exercise(count, enabled));
        }
    }
    println!(
        "{}",
        serde_json::json!({"unit":"nanoseconds","endpoint":"Painter::build with layout already built and previous Layout alive","samples":SAMPLES,"warmups":WARMUPS,"order":"disabled batch then enabled batch","observations":observations})
    );
}

fn requests(manager: &Manager) -> u64 {
    manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "subsequences")
        .unwrap()
        .stats
        .requests
}
