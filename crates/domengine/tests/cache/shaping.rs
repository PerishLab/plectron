use super::{constraints, html};
use ::fixtures::Text;
use domengine::{Input, Layout, Pipeline, Shaper, Sheet};
use shared::cache::Manager;
use std::cell::Cell;
use std::rc::Rc;

struct Dynamic {
    revision: Rc<Cell<u64>>,
    unstable: bool,
}
impl Shaper for Dynamic {
    fn revision(&self) -> u64 {
        self.revision.get()
    }
    fn select(
        &self,
        source: &str,
        font: &textengine::Request,
        spacing: usize,
    ) -> Result<Vec<textengine::Selection>, textengine::Fault> {
        let selected = Text.select(source, font, spacing + self.revision.get() as usize * 16)?;
        if self.unstable {
            self.revision.set(self.revision.get() + 1);
        }
        Ok(selected)
    }
}

#[test]
fn invalidation() {
    let document = html::parse("<p>AB</p>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let revision = Rc::new(Cell::new(0));
    let mut pipeline = Pipeline::new(Dynamic {
        revision: revision.clone(),
        unstable: false,
    });
    let before = pipeline.build(&input, constraints(32)).unwrap();
    revision.set(1);
    let after = pipeline.build(&input, constraints(32)).unwrap();
    assert!(pipeline.inspect().reset);
    assert_ne!(before.fragments(), after.fragments());
    let expected = Layout::build(
        &input,
        constraints(32),
        &Dynamic {
            revision: revision.clone(),
            unstable: false,
        },
    )
    .unwrap();
    assert_eq!(after.fragments(), expected.fragments());
    let mut unstable = Pipeline::new(Dynamic {
        revision,
        unstable: true,
    });
    assert!(
        unstable
            .build(&input, constraints(32))
            .unwrap_err()
            .to_string()
            .contains("shaper changed")
    );
    assert_eq!(unstable.inspect().entries, 0);
}

#[test]
fn observation() {
    let document = html::parse("<p>office affinity</p>").unwrap();
    let sheet = Sheet::parse("p {font-family: lato; overflow-wrap: anywhere;}").unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let mut pipeline = Pipeline::new(Text);
    pipeline.build(&input, constraints(32)).unwrap();
    let cold = pipeline.inspect().shaping;
    assert_eq!(cold.preparations, 1);
    assert!(cold.shapes > 1);
    assert!(cold.candidates > cold.materialized);
    assert!(cold.materialized > 0);
    assert!(cold.bytes > 15);
    pipeline.build(&input, constraints(32)).unwrap();
    let warm = pipeline.inspect().shaping;
    assert_eq!(
        warm.preparations + warm.shapes + warm.candidates + warm.bytes + warm.materialized,
        0
    );
    pipeline.build(&input, constraints(48)).unwrap();
    assert_eq!(pipeline.inspect().shaping.preparations, 0);
    assert!(pipeline.inspect().shaping.materialized > 0);
}

#[test]
fn attachments() {
    let document = html::parse("<p>stable text</p>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let mut pipeline = Pipeline::new(Text);
    let first = pipeline.build(&input, constraints(128)).unwrap();
    let next = pipeline.build(&input, constraints(128)).unwrap();
    let attached = first
        .snapshot()
        .metrics()
        .filter_map(|metric| metric.attachment().map(|value| (metric.id, value)))
        .collect::<Vec<_>>();
    assert!(!attached.is_empty());
    assert!(attached.iter().all(|(id, attachment)| {
        next.snapshot().metric(*id).unwrap().attachment() == Some(*attachment)
    }));
}

#[test]
fn deduplication() {
    let document = html::parse("<div><p>same</p><p>same</p></div>").unwrap();
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("layout.retention", false.into());
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    pipeline.build(&input, constraints(128)).unwrap();
    let requests = || {
        manager
            .inspect()
            .into_iter()
            .find(|cache| cache.name == "attachments")
            .unwrap()
            .stats
            .requests
    };
    let before = requests();
    pipeline.build(&input, constraints(128)).unwrap();
    assert_eq!(requests() - before, 1);
    assert_eq!(pipeline.inspect().hits, 1);
}
