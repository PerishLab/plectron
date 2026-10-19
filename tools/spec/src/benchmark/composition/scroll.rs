use super::super::elapsed;
use crate::Result;
use serde::Serialize;
use std::hint::black_box;
use std::time::Instant;
use webengine::resource::{Provider, Request, Response};
use webengine::{Fault, Page, Snapshot, change, mapping};

pub type Report = crate::benchmark::Envelope<crate::benchmark::Native, Sample>;

#[derive(Serialize)]
pub struct Sample {
    repetition: usize,
    rows: usize,
    fragments: usize,
    iterations: usize,
    wall: u64,
    work: change::Work,
    mapping: mapping::Work,
    materialization: graphics::shared::Materialization,
}

struct Source {
    html: Vec<u8>,
}

impl Source {
    fn new(rows: usize) -> Self {
        let mut body = String::from(
            "<html><head><link rel='stylesheet' href='style.css'></head><body><div id='timeline'>",
        );
        for _ in 0..rows {
            body.push_str("<div class='row'>x</div>");
        }
        body.push_str("</div></body></html>");
        Self {
            html: body.into_bytes(),
        }
    }
}

impl Provider for Source {
    fn request(&mut self, request: &Request) -> std::result::Result<Response, Fault> {
        let (mime, body) = match request.url.path() {
            "/index.html" => ("text/html", self.html.clone()),
            "/style.css" => (
                "text/css",
                b"body {width: 400px; height: 240px; overflow: hidden;} #timeline {height: 160px; overflow: auto;} .row {height: 40px;}".to_vec(),
            ),
            _ => return Err(Fault("unexpected scroll benchmark resource".into())),
        };
        Ok(Response {
            status: 200,
            mime: mime.into(),
            body,
        })
    }
}

pub fn run(platform: (&str, &str), rows: &[usize], iterations: usize) -> Result<Report> {
    if rows.is_empty() || rows.contains(&0) || iterations == 0 {
        return Err("scroll benchmark dimensions must be positive".into());
    }
    let mut samples = Vec::with_capacity(rows.len() * 7);
    for repetition in 0..7 {
        for index in 0..rows.len() {
            let rows = rows[(repetition + index) % rows.len()];
            samples.push(sample(rows, iterations, repetition)?);
        }
    }
    Ok(Report::native("nanoseconds", platform, samples))
}

fn sample(rows: usize, iterations: usize, repetition: usize) -> Result<Sample> {
    let mut page = Page::new(
        "app://scroll/index.html",
        domengine::Extent {
            width: 400,
            height: 240,
        },
    )
    .map_err(error)?;
    page.load(&mut Source::new(rows)).map_err(error)?;
    let initial = page.inspect();
    let (scene, space) = target(&initial)?;
    page.pan(scene, space, 80).map_err(error)?;
    for index in 0..32 {
        page.pan(scene, space, direction(index)).map_err(error)?;
    }
    let before = page.inspect();
    let start = Instant::now();
    for index in 0..iterations {
        page.pan(scene, space, direction(index)).map_err(error)?;
    }
    let wall = elapsed(start);
    let after = page.inspect();
    let work = after.scrollwork.since(before.scrollwork);
    let mapping = after.mapping.since(before.mapping);
    let materialization = after.materialization.since(before.materialization);
    let projected = work.updates == iterations as u64 && work.incremental == iterations as u64;
    let stable = work.positions == iterations as u64 && work.rebuilds == 0;
    let framed = mapping.updates == iterations as u64 && mapping.revisions == iterations as u64;
    let deferred = mapping.spaces == 0 && materialization.materializations == 0;
    let scoped = mapping.offsets == iterations as u64 && mapping.affected > 0 && deferred;
    let accepted = [projected, stable, framed, scoped]
        .into_iter()
        .all(|pass| pass);
    if !accepted {
        return Err("scroll benchmark left the stable incremental path".into());
    }
    black_box(after.visual.offset(space));
    Ok(Sample {
        repetition,
        rows,
        fragments: initial.fragments.len(),
        iterations,
        wall,
        work,
        mapping,
        materialization,
    })
}

fn target(snapshot: &Snapshot) -> Result<(u64, u64)> {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| {
            node.attributes
                .get("id")
                .is_some_and(|value| value == "timeline")
        })
        .ok_or("scroll benchmark target absent")?;
    let root = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .ok_or("scroll benchmark fragment absent")?;
    let mapping = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(root.space) && mapping.range.is_some())
        .ok_or("scroll benchmark space absent")?;
    Ok((
        snapshot.scene.ok_or("scroll benchmark scene absent")?,
        mapping.id,
    ))
}

fn direction(index: usize) -> i64 {
    if index.is_multiple_of(2) { 8 } else { -8 }
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
