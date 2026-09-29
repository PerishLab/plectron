use ::fixtures::Text;
use domengine::layout::State;
use domengine::{Available, Constraints, Content, Extent, Input, Layout, Pipeline, Sheet, html};
use shared::cache::Manager;

#[test]
fn sizing() {
    let document = html::parse("<p id='text'>ABCD</p>").unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    let measure = |width| {
        Layout::build(
            &input,
            Constraints {
                available: [width, Available::Max],
                known: [None; 2],
            },
            &Text,
        )
        .unwrap()
    };
    assert_eq!(
        measure(Available::Min).extent(),
        Extent {
            width: 16,
            height: 64
        }
    );
    assert_eq!(
        measure(Available::Max).extent(),
        Extent {
            width: 64,
            height: 16
        }
    );
    assert_eq!(
        measure(Available::Definite(32)).extent(),
        Extent {
            width: 32,
            height: 32
        }
    );
    let mut constraints = Constraints::viewport(Extent {
        width: 200,
        height: 100,
    });
    constraints.known = [Some(32), Some(20)];
    let layout = Layout::build(&input, constraints, &Text).unwrap();
    assert!(layout.snapshot().impact().is_none());
    assert_eq!(layout.fragments()[0].rect.width, 32);
    assert_eq!(layout.fragments()[0].rect.height, 20);
    assert_eq!(
        layout.extent(),
        Extent {
            width: 200,
            height: 100
        }
    );
    assert_eq!(layout.hit(4, 24), None);
    assert_eq!(layout.hit(4, 18), document.find("text"));
}

#[test]
fn boundaries() {
    let document =
        html::parse("<div id='box' style='width: 20px; padding: 4px; margin: 3px;'>ABCD</div>")
            .unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    let layout = Layout::build(
        &input,
        Constraints {
            available: [Available::Max; 2],
            known: [None; 2],
        },
        &Text,
    )
    .unwrap();
    assert_eq!(
        layout.extent(),
        Extent {
            width: 34,
            height: 78
        }
    );
    assert_eq!(
        layout
            .fragments()
            .iter()
            .filter(|fragment| matches!(fragment.content, Content::Line))
            .count(),
        4
    );
    assert!(
        Layout::build(
            &input,
            Constraints {
                available: [Available::Definite(usize::MAX), Available::Max],
                known: [None; 2],
            },
            &Text
        )
        .is_err()
    );
    assert!(
        Layout::build(
            &input,
            Constraints {
                available: [Available::Max; 2],
                known: [Some(usize::MAX), None],
            },
            &Text
        )
        .is_err()
    );
}

#[test]
fn propagation() {
    let document = html::parse("<div><p>ABCD</p><p>EFGH</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::parse("").unwrap()).unwrap();
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("layout.impact", true.into());
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    pipeline
        .build(
            &input,
            Constraints::viewport(Extent {
                width: 32,
                height: 64,
            }),
        )
        .unwrap();
    let wide = pipeline
        .build(
            &input,
            Constraints::viewport(Extent {
                width: 64,
                height: 64,
            }),
        )
        .unwrap();
    assert_eq!(wide.snapshot().impact().unwrap().seeds().len(), 1);
    assert_eq!(
        wide.snapshot().impact().unwrap().affected().len(),
        wide.tree().nodes().len()
    );
    assert!(wide.snapshot().impact().unwrap().missed().is_empty());
}

#[test]
fn arena() {
    let document = html::parse("<div><p>A</p><p>B</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::parse("").unwrap()).unwrap();
    let tree = input.tree();
    let root = tree.root();
    assert_eq!(tree.entity(root), tree.roots().next());
    let children = tree.flow(root).unwrap().collect::<Vec<_>>();
    assert!(!children.is_empty());
    assert!(children.iter().all(|id| {
        tree.entity(*id)
            .is_some_and(|entity| entity.parent == Some(root))
    }));

    let layout = Layout::build(
        &input,
        Constraints::viewport(Extent {
            width: 128,
            height: 128,
        }),
        &Text,
    )
    .unwrap();
    assert_eq!(
        layout
            .snapshot()
            .metrics()
            .map(|metric| metric.id)
            .collect::<Vec<_>>(),
        tree.nodes()
            .iter()
            .map(|entity| entity.id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn states() {
    let document = html::parse("<div><div id='limit'><p>text</p></div></div>").unwrap();
    let limit = document.find("limit").unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("#limit { width: 64px; height: 32px; contain: size layout paint; }").unwrap(),
    )
    .unwrap();
    let mut pipeline = Pipeline::new(Text);
    pipeline.demand(Some([].into()));
    let viewport = Constraints::viewport(Extent {
        width: 128,
        height: 128,
    });
    let layout = pipeline.build(&input, viewport).unwrap();
    let boundary = layout.tree().source(limit).next().unwrap().id;
    let states = layout.snapshot().states();
    assert_eq!(states.count(), layout.tree().nodes().len());
    let mut deferred = 0;
    let mut blocked = 0;
    for (entity, state) in layout.tree().nodes().iter().zip(states.iter()) {
        match state {
            State::Resolved(_) => assert_ne!(entity.id, boundary),
            State::Deferred(metric) => {
                assert_eq!(metric.id, boundary);
                deferred += 1;
            }
            State::Blocked(owner) => {
                assert_eq!(*owner, boundary);
                blocked += 1;
            }
        }
    }
    assert_eq!(deferred, 1);
    assert!(blocked > 0);
    assert_eq!(layout.snapshot().delta().added().len(), states.count());
    assert!(layout.snapshot().delta().retired().is_empty());
}

struct Large;
impl domengine::Shaper for Large {
    fn revision(&self) -> u64 {
        0
    }
    fn select(
        &self,
        source: &str,
        font: &textengine::Request,
        spacing: usize,
    ) -> Result<Vec<textengine::Selection>, textengine::Fault> {
        domengine::Shaper::select(&Text, source, font, spacing + 4080)
    }
}
#[test]
fn unbounded() {
    let document = html::parse(&format!("<p>{}</p>", "A".repeat(257))).unwrap();
    let input = Input::capture(
        &document,
        &Sheet::parse("document {overflow-wrap: anywhere;}").unwrap(),
    )
    .unwrap();
    let constraints = Constraints {
        available: [Available::Max; 2],
        known: [None; 2],
    };
    assert!(Layout::build(&input, constraints, &Large).is_err());
    let constrained = Constraints {
        available: [Available::Definite(4096), Available::Max],
        known: [None; 2],
    };
    assert_eq!(
        Layout::build(&input, constrained, &Large).unwrap().extent(),
        Extent {
            width: 4096,
            height: 4112
        }
    );
}
