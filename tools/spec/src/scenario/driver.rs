use super::{
    Case, check, elapsed,
    rendering::Renderer,
    report::{Failure, Run, Stage},
    settings::Settings,
    source::Source,
};
use crate::Result;
use std::time::Instant;
use webengine::{Page, Snapshot};

pub(super) fn run(
    case: &Case,
    settings: &Settings,
    renderer: &mut Renderer,
    config: config::Store,
) -> Result<Run> {
    let mut source = Source::configured(&config).map_err(error)?;
    let extent = graphics::shared::Extent {
        width: case.viewport[0],
        height: case.viewport[1],
    };
    let mut page = Page::configured(
        &format!("app://scenarios/{}", case.entry),
        extent,
        config.clone(),
    )
    .map_err(error)?;
    let start = Instant::now();
    let loaded = page
        .load(&mut source)
        .and_then(|()| super::settings::prepare(&mut page, &config, &case.region));
    let load = elapsed(start);
    let snapshot = page.inspect();
    let startup = snapshot.milestones;
    let failure = loaded
        .err()
        .map(|error| failure(snapshot, case, "load", error.to_string()));
    let mut report = Run {
        config: config.inspect(),
        load,
        startup,
        loading: page.loading(),
        assets: Default::default(),
        stages: Vec::new(),
        closed: false,
        failure,
    };
    if report.failure.is_none() {
        exercise((&mut page, renderer), (case, settings), &mut report);
    }
    page.close();
    let closed = page.inspect();
    if closed.state != webengine::State::Closed
        || !closed.nodes.is_empty()
        || page.frame().is_some()
    {
        return Err("closed scenario retained executable page authority".into());
    }
    if page.evaluate("0;").is_ok() {
        return Err("closed page accepted evaluation".into());
    }
    report.assets = source.assets().map_err(error)?;
    report.closed = true;
    Ok(report)
}
fn failure(snapshot: Snapshot, case: &Case, action: &str, reason: String) -> Failure {
    Failure {
        action: action.into(),
        reason,
        nodes: snapshot.nodes.len(),
        records: check::View(&snapshot).count(&case.prefix),
    }
}
fn exercise(context: (&mut Page, &mut Renderer), config: (&Case, &Settings), report: &mut Run) {
    let (case, settings) = config;
    let count = settings.count;
    let mut actions = vec!["seed", "steady"];
    actions.extend(case.actions.iter().map(String::as_str));
    for action in actions {
        let range = match action {
            "seed" => 0..count,
            "edit" => 0..settings.edits,
            _ => count..count + count.div_ceil(4),
        };
        match observe::call(format!("action.{action}"), || {
            step((context.0, context.1), case, action, range)
        }) {
            Ok(stage) => report.stages.push(stage),
            Err(reason) => {
                report.failure = Some(failure(context.0.inspect(), case, action, reason));
                return;
            }
        }
    }
}
fn step(
    context: (&mut Page, &mut Renderer),
    case: &Case,
    action: &str,
    range: std::ops::Range<usize>,
) -> Result<Stage> {
    let (page, renderer) = context;
    let count = range.len();
    let indices = serde_json::to_string(&range.collect::<Vec<_>>()).map_err(error)?;
    let before = page.inspect();
    let start = Instant::now();
    let timing = apply(page, (&before, &case.region), action, &indices)?;
    let mut stage = record(page, renderer, action, (timing.0, timing.1, start))?;
    let after = page.inspect();
    stage.computation = after.computation.since(before.computation);
    stage.activity = after.activity.since(before.activity);
    stage.bindings = after.bindings.since(before.bindings);
    if before.scene == after.scene {
        stage.update = None;
        stage.reuse = None;
        stage.painting = None;
        stage.routing = None;
    }
    stage.records = check::View(&after).count(&case.prefix);
    stage.geometry = check::capture(&after, case)?;
    if action == "seed" {
        check::View(&after).seed(&case.prefix, count)?;
    } else {
        check::step((&before, &after), case, action, count)?;
    }
    if matches!(action, "hide" | "show") {
        demand(&after, case, action == "hide")?;
    }
    Ok(stage)
}

fn apply(
    page: &mut Page,
    view: (&Snapshot, &str),
    action: &str,
    indices: &str,
) -> Result<(u64, u64)> {
    let script = match action {
        "seed" => format!("scenario.seed({indices});"),
        "steady" | "scroll" => String::new(),
        _ => format!(
            "scenario.run({}, {indices});",
            serde_json::to_string(action).map_err(error)?
        ),
    };
    let start = Instant::now();
    if action == "scroll" {
        scroll(page, view.0, view.1)?;
    } else if !script.is_empty() {
        page.evaluate(&script).map_err(error)?;
    }
    let evaluation = elapsed(start);
    let start = Instant::now();
    page.render().map_err(error)?;
    Ok((evaluation, elapsed(start)))
}
fn scroll(page: &mut Page, snapshot: &Snapshot, name: &str) -> Result<()> {
    let target = check::View(snapshot).node(name)?;
    let space = check::View(snapshot).scroller(name)?;
    let region = snapshot
        .visible
        .iter()
        .find(|region| region.node == Some(target.id))
        .ok_or("scroll region has not been rendered")?;
    let rect = region.rect.intersect(region.clip);
    if rect.width == 0 || rect.height == 0 {
        return Err("scroll region is outside viewport".into());
    }
    let range = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.id == space)
        .and_then(|mapping| mapping.range)
        .ok_or("scroll range missing")?;
    let delta = if snapshot.visual.offset(space).saturating_add(48) <= range {
        48
    } else {
        -48
    };
    page.wheel(rect.x + rect.width / 2, rect.y + rect.height / 2, delta)
        .map(|_| ())
        .map_err(error)
}
fn record(
    page: &mut Page,
    renderer: &mut Renderer,
    action: &str,
    timing: (u64, u64, Instant),
) -> Result<Stage> {
    let capture;
    let rendering = if let Some(frame) = page.frame() {
        let (report, observed) = renderer.render(&frame, timing.2).map_err(error)?;
        capture = observed;
        page.acknowledge(frame.id()).map_err(error)?;
        Some(report)
    } else {
        capture = renderer.repeat(timing.2);
        None
    };
    let inspection = Instant::now();
    let snapshot = page.inspect();
    let inspection = elapsed(inspection);
    Ok(Stage {
        activity: Default::default(),
        bindings: Default::default(),
        computation: Default::default(),
        relevance: snapshot.relevance,
        action: action.into(),
        trigger: match action {
            "scroll" => "wheel",
            "steady" => "checkpoint",
            _ => "application",
        },
        evaluation: timing.0,
        publication: timing.1,
        inspection,
        publishing: page.timing(),
        total: timing.0.saturating_add(timing.1).saturating_add(
            rendering
                .as_ref()
                .map_or(0, |r| r.preparation.saturating_add(r.completion)),
        ),
        rendering,
        capture,
        geometry: Default::default(),
        nodes: snapshot.nodes.len(),
        records: 0,
        depth: check::View(&snapshot).depth(),
        characters: snapshot
            .nodes
            .iter()
            .filter(|n| n.kind == "text")
            .map(|n| n.content.chars().count())
            .sum(),
        visible: snapshot
            .visible
            .iter()
            .filter(|r| {
                let rect = r.rect.intersect(r.clip);
                rect.width > 0 && rect.height > 0
            })
            .count(),
        performance: snapshot.performance,
        update: Some(snapshot.update),
        reuse: Some(snapshot.reuse),
        painting: Some(snapshot.painting),
        routing: Some(snapshot.routing),
        caches: snapshot.caches,
        images: snapshot.images,
    })
}
fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}

fn demand(snapshot: &Snapshot, case: &Case, hidden: bool) -> Result<()> {
    let node = check::View(snapshot).node(&case.region)?.id;
    let opacity = snapshot.fragments.iter().find_map(|fragment| {
        if fragment.node != Some(node) {
            return None;
        }
        match fragment.content {
            domengine::Content::Box { opacity, .. } => Some(opacity),
            _ => None,
        }
    });
    if opacity.map(|value| value == 0) != Some(hidden) {
        return Err("scenario visibility differs from requested demand".into());
    }
    Ok(())
}
