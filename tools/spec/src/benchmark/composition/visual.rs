use super::super::elapsed;
use crate::Result;
use graphics::shared::Extent;
use serde::Serialize;
use std::time::Instant;
use webengine::{
    Fault, Page, Snapshot,
    resource::{Provider, Request, Response},
};

const LOGICAL: usize = 100_000;
const LIMIT: usize = 64;

#[derive(Serialize)]
pub struct Report {
    unit: &'static str,
    architecture: &'static str,
    system: &'static str,
    debug: bool,
    iterations: usize,
    samples: Vec<Sample>,
}

#[derive(Serialize)]
struct Sample {
    repetition: usize,
    startup: u64,
    milestones: webengine::Milestones,
    initial: State,
    jump: Stage,
    resize: Stage,
    scroll: Stage,
    end: Stage,
    objects: usize,
    nodes: usize,
}

#[derive(Serialize)]
struct State {
    logical: usize,
    start: usize,
    end: usize,
    live: usize,
    nodes: usize,
    fragments: usize,
}

#[derive(Serialize)]
struct Stage {
    wall: u64,
    state: State,
    computation: webengine::Computation,
    bindings: webengine::Bindings,
    scrolling: webengine::ScrollWork,
    mapping: webengine::MappingWork,
    materialization: graphics::shared::Materialization,
}

#[derive(Clone)]
struct Application(fixtures::Application);

impl Provider for Application {
    fn request(&mut self, request: &Request) -> std::result::Result<Response, Fault> {
        let resource = self.0.load(request.url.path()).map_err(Fault)?;
        request.accept(Response {
            status: 200,
            mime: resource.mime,
            body: resource.body,
        })
    }
}

struct Bench {
    page: Page,
}

impl Bench {
    fn present(&mut self) -> Result<()> {
        if let Some(frame) = self.page.frame() {
            self.page.acknowledge(frame.id()).map_err(error)?;
        }
        Ok(())
    }

    fn action(&mut self, script: &str) -> Result<Stage> {
        let before = self.page.inspect();
        let started = Instant::now();
        self.page.evaluate(script).map_err(error)?;
        self.page.render().map_err(error)?;
        self.present()?;
        self.stage(&before, started)
    }

    fn scrolling(&mut self, iterations: usize) -> Result<Stage> {
        let before = self.page.inspect();
        let started = Instant::now();
        for index in 0..iterations {
            if !self.page.wheel(100, 200, 80).map_err(error)?.applied() {
                return Err(format!("visual scroll was blocked at iteration {index}"));
            }
        }
        self.page.render().map_err(error)?;
        self.present()?;
        self.stage(&before, started)
    }

    fn stage(&mut self, before: &Snapshot, started: Instant) -> Result<Stage> {
        let wall = elapsed(started);
        let after = self.page.inspect();
        Ok(Stage {
            wall,
            state: self.state(&after)?,
            computation: after.computation.since(before.computation),
            bindings: after.bindings.since(before.bindings),
            scrolling: after.scrollwork.since(before.scrollwork),
            mapping: after.mapping.since(before.mapping),
            materialization: after.materialization.since(before.materialization),
        })
    }

    fn state(&mut self, snapshot: &Snapshot) -> Result<State> {
        let values = self
            .page
            .evaluate("scenario.range();")
            .map_err(error)?
            .split(':')
            .map(|value| value.parse::<usize>().map_err(error))
            .collect::<Result<Vec<_>>>()?;
        let [start, end, logical] = values.as_slice() else {
            return Err("visual range is malformed".into());
        };
        let live = snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.attributes
                    .get("id")
                    .is_some_and(|id| id.starts_with("visual-record-"))
            })
            .count();
        Ok(State {
            logical: *logical,
            start: *start,
            end: *end,
            live,
            nodes: snapshot.nodes.len(),
            fragments: snapshot.fragments.len(),
        })
    }
}

pub fn run(platform: (&'static str, &'static str), iterations: usize) -> Result<Report> {
    if iterations == 0 || iterations > 1_048_576 {
        return Err("visual iterations are outside the supported range".into());
    }
    let mut samples = Vec::with_capacity(7);
    for repetition in 0..7 {
        samples.push(sample(repetition, iterations)?);
    }
    Ok(Report {
        unit: "nanoseconds",
        architecture: platform.0,
        system: platform.1,
        debug: cfg!(debug_assertions),
        iterations,
        samples,
    })
}

fn sample(repetition: usize, iterations: usize) -> Result<Sample> {
    let started = Instant::now();
    let mut page = Page::new(
        "app://visual/index.html",
        Extent {
            width: 720,
            height: 640,
        },
    )
    .map_err(error)?;
    page.load(&mut Application(fixtures::Application::new("visual")))
        .map_err(error)?;
    let startup = elapsed(started);
    let snapshot = page.inspect();
    let mut bench = Bench { page };
    bench.present()?;
    let initial = bench.state(&snapshot)?;
    valid("initial", &initial, |state| state.end == LOGICAL)?;
    let jump = bench.action("scenario.jump(50000);")?;
    valid("jump", &jump.state, |state| {
        state.start <= 50_000 && 50_000 < state.end
    })?;
    let resize = bench.action("scenario.grow(50001);")?;
    let before = bench.page.inspect();
    bench.page.evaluate("scenario.start();").map_err(error)?;
    bench.page.render().map_err(error)?;
    bench.present()?;
    let before = bench.page.inspect().computation.since(before.computation);
    if before.layouts == 0 {
        return Err("visual start did not rebuild layout".into());
    }
    let scroll = bench.scrolling(iterations)?;
    valid("scroll", &scroll.state, |_| true)?;
    let end = bench.action("scenario.end();")?;
    valid("end", &end.state, |state| state.end == LOGICAL)?;
    let collection = bench.page.collect().map_err(error)?;
    bench.page.close();
    Ok(Sample {
        repetition,
        startup,
        milestones: snapshot.milestones,
        initial,
        jump,
        resize,
        scroll,
        end,
        objects: collection.objects,
        nodes: collection.nodes,
    })
}

fn valid(name: &str, state: &State, condition: impl FnOnce(&State) -> bool) -> Result<()> {
    let identity = state.logical == LOGICAL;
    let population = state.live > 0 && state.live < LIMIT;
    let bounded = state.end - state.start < LIMIT;
    if ![identity, population, bounded, condition(state)]
        .into_iter()
        .all(|valid| valid)
    {
        return Err(format!(
            "visual {name} window left its bounded contract: logical={} range={}..{} live={}",
            state.logical, state.start, state.end, state.live
        ));
    }
    Ok(())
}

fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}
