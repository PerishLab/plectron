use ::fixtures::Text;
use domengine::{Constraints, Content, Extent, Input, Layout, Shaper, Sheet, html};

mod coverage;

fn constraints(width: usize) -> Constraints {
    Constraints::viewport(Extent { width, height: 16 })
}

#[test]
fn fragments() {
    let mut document = html::parse("<button id='button'>ABCD</button>").unwrap();
    let button = document.find("button").unwrap();
    let node = document.entry(button).unwrap().children[0];
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    document
        .attribute(button, "style", "display: none;")
        .unwrap();
    drop(document);
    let layout = Layout::build(&input, constraints(32), &Text).unwrap();
    let fragments: Vec<_> = layout
        .fragments()
        .iter()
        .filter(|fragment| fragment.node == Some(node.id()))
        .collect();
    assert_eq!(fragments.len(), 2);
    assert_ne!(fragments[0].id, fragments[1].id);
    assert_eq!(fragments[0].rect.y, 0);
    assert_eq!(fragments[1].rect.y, 16);
    assert_eq!(layout.extent().height, 32);
    assert_eq!(layout.hit(8, 20), Some(button));
    assert_eq!(
        layout
            .fragments()
            .iter()
            .filter(|fragment| matches!(fragment.content, Content::Line) && fragment.node.is_none())
            .count(),
        2
    );
    let image = domengine::raster(
        Extent {
            width: 32,
            height: 32,
        },
        &layout.paint(),
    )
    .unwrap();
    assert!(
        image.pixels[16 * 32..]
            .iter()
            .any(|pixel| *pixel != 0xffffff)
    );
}

#[test]
fn shaping() {
    let run = Text
        .shape("fi雪A", &textengine::Request::default(), 0)
        .unwrap();
    assert_eq!(run.clusters.len(), 3);
    assert_eq!(run.clusters[0].source, 0..2);
    assert_eq!(run.glyphs[0].id, 128);
    assert_eq!(run.clusters[0].advance, 24);
    assert_eq!(run.clusters[1].source, 2..5);
    let document = html::parse("<p>fiA</p>").unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    let layout = Layout::build(&input, constraints(24), &Text).unwrap();
    let runs: Vec<_> = layout
        .fragments()
        .iter()
        .filter_map(|fragment| match &fragment.content {
            Content::Text {
                glyphs, clusters, ..
            } => Some((glyphs, clusters, fragment.origin())),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].0.len(), 1);
    assert_eq!(runs[0].1.len(), 1);
    assert_eq!(runs[0].1[0], 0..2);
    assert_eq!(
        runs[1].0[0].y + graphics::shared::Scalar::from(runs[1].2[1] as i64),
        16
    );
}

struct Wide;
impl Shaper for Wide {
    fn revision(&self) -> u64 {
        0
    }
    fn select(
        &self,
        source: &str,
        font: &textengine::Request,
        spacing: usize,
    ) -> Result<Vec<textengine::Selection>, textengine::Fault> {
        Text.select(source, font, spacing + 16)
    }
}

#[test]
fn metrics() {
    let document = html::parse("<p>AB</p>").unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    let normal = Layout::build(&input, constraints(32), &Text).unwrap();
    let wide = Layout::build(&input, constraints(32), &Wide).unwrap();
    assert_eq!(normal.extent().height, 16);
    assert_eq!(wide.extent().height, 32);
    assert_eq!(normal.revision(), wide.revision());
}

struct Broken(bool);
impl Shaper for Broken {
    fn revision(&self) -> u64 {
        0
    }
    fn select(
        &self,
        source: &str,
        font: &textengine::Request,
        spacing: usize,
    ) -> Result<Vec<textengine::Selection>, textengine::Fault> {
        let mut selected = Text.select(source, font, spacing)?;
        if self.0 {
            selected[0].source.end = 1;
        } else {
            selected[0].font.size = 4097.into();
        }
        Ok(selected)
    }
}

#[test]
fn invalid() {
    let document = html::parse("<p>雪</p>").unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    assert!(Layout::build(&input, constraints(32), &Broken(true)).is_err());
    assert!(Layout::build(&input, constraints(32), &Broken(false)).is_err());
}

mod tree {
    use domengine::{
        Capture, Constraints, Document, Extent, Input, Pipeline, Sheet, html,
        layout::{Bridge, Role},
        style::Resolver,
    };
    use fixtures::Text;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    fn setup(source: &str) -> (Document, Capture, Pipeline<Text>) {
        let cache = shared::cache::Manager::default();
        let sheet = Sheet::parse("").unwrap();
        (
            html::parse(source).unwrap(),
            Capture::new(Resolver::managed(sheet, &cache).unwrap()),
            Pipeline::managed(Text, &cache).unwrap(),
        )
    }

    fn publish(
        document: &Document,
        capture: &mut Capture,
        pipeline: &mut Pipeline<Text>,
    ) -> domengine::Layout {
        capture
            .update(document, |input| {
                pipeline.build(
                    input,
                    Constraints::viewport(Extent {
                        width: 128,
                        height: 128,
                    }),
                )
            })
            .unwrap()
    }

    #[test]
    fn topology() {
        let input = Input::capture(
            &html::parse("<div><span>A</span><span>B</span></div>").unwrap(),
            &Sheet::parse("").unwrap(),
        )
        .unwrap();
        let tree = input.tree();
        let mut seen = BTreeSet::new();
        assert_eq!(tree.roots().count(), 1);
        for node in tree.nodes() {
            assert!(seen.insert(node.id));
            if let Some(parent) = node.parent {
                assert!(seen.contains(&parent));
            }
        }
        assert!(tree.nodes().iter().any(|node| node.role == Role::Anonymous));
    }

    #[test]
    fn identity() {
        let (mut document, mut capture, mut pipeline) =
            setup("<div id='root'><p id='stable'>A</p></div>");
        let stable = document.find("stable").unwrap();
        let text = document.entry(stable).unwrap().children[0];
        let first = publish(&document, &mut capture, &mut pipeline);
        let sid = first.tree().source(stable).next().unwrap().id;
        let tid = first.tree().source(text).next().unwrap().id;
        let age = first.tree().generation();
        document.replace(text, "Changed").unwrap();
        let second = publish(&document, &mut capture, &mut pipeline);
        assert_ne!(second.tree().generation(), age);
        assert_eq!(second.tree().source(stable).next().unwrap().id, sid);
        assert_eq!(second.tree().source(text).next().unwrap().id, tid);
        let added = document.element("p").unwrap();
        document.replace(added, "B").unwrap();
        document
            .append(document.find("root").unwrap(), added)
            .unwrap();
        let third = publish(&document, &mut capture, &mut pipeline);
        assert_eq!(third.tree().source(stable).next().unwrap().id, sid);
        assert_ne!(third.tree().source(added).next().unwrap().id, sid);
    }

    #[test]
    fn mapping() {
        let (document, mut capture, mut pipeline) = setup("<input>");
        let input = document.entry(document.root()).unwrap().children[0];
        let layout = publish(&document, &mut capture, &mut pipeline);
        let mapped = layout.tree().source(input).collect::<Vec<_>>();
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].role, Role::Source);
        assert_eq!(mapped[1].role, Role::Editor);
        assert_ne!(mapped[0].id, mapped[1].id);
        assert_eq!(layout.tree().revision(), layout.revision());
        let usage = layout.usage();
        assert_eq!(
            usage.active().collect::<Vec<_>>(),
            [Bridge::Measure, Bridge::Fragment]
        );
        assert_eq!(usage.calls(Bridge::Measure), 1);
        layout.spatial();
        layout.paint();
        layout.hit(0, 0);
        layout.effects(&[]);
        let usage = layout.usage();
        assert_eq!(usage.calls(Bridge::Space), 1);
        assert_eq!(usage.calls(Bridge::Paint), 1);
        assert_eq!(usage.calls(Bridge::Hit), 1);
        assert_eq!(usage.calls(Bridge::Effect), 1);
    }

    #[test]
    fn snapshot() {
        let document = html::parse("<div><p>A</p><p>B</p></div>").unwrap();
        let input = Input::capture(&document, &Sheet::parse("").unwrap()).unwrap();
        let mut engine = Pipeline::new(Text);
        let narrow = engine.build(&input, super::constraints(32)).unwrap();
        let wide = engine.build(&input, super::constraints(64)).unwrap();
        assert!(Arc::ptr_eq(narrow.tree(), wide.tree()));
        assert_ne!(narrow.snapshot().generation(), wide.snapshot().generation());
        assert_eq!(
            narrow.snapshot().metrics().count(),
            narrow.tree().nodes().len()
        );
        assert!(narrow.snapshot().metrics().all(|metric| {
            narrow
                .tree()
                .nodes()
                .iter()
                .any(|node| node.id == metric.id)
        }));
        let root = narrow.tree().roots().next().unwrap().id;
        let child = narrow.tree().nodes()[1].id;
        assert_eq!(narrow.snapshot().metric(root).unwrap().size()[0], 32);
        assert_eq!(wide.snapshot().metric(root).unwrap().size()[0], 64);
        assert!(wide.snapshot().delta().changed().contains(&root));
        assert!(wide.snapshot().delta().resized().contains(&root));
        assert!(wide.snapshot().delta().resized().contains(&child));
        assert!(wide.snapshot().delta().retired().is_empty());
    }
}
