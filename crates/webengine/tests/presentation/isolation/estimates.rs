use super::*;

fn content() -> String {
    let mut html =
        String::from("<body><div id='list' style='width:64px; height:32px; overflow:auto;'>");
    for index in 0..20 {
        let text = if index == 19 {
            "a final row with enough words to wrap across several lines"
        } else {
            "row"
        };
        html.push_str(&format!(
            "<div id='row{index}' style='contain:size layout paint; contain-intrinsic-height:auto 20px;'>{text}</div>"
        ));
    }
    html.push_str("</div></body>");
    html
}

fn anchor(snapshot: &webengine::Snapshot, id: &str) -> (usize, i128) {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .unwrap();
    let root = snapshot
        .fragments
        .iter()
        .find(|fragment| {
            fragment.node == Some(node.id)
                && matches!(
                    fragment.content,
                    domengine::Content::Box {
                        scroll: Some(_),
                        ..
                    }
                )
        })
        .unwrap();
    let space = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(root.space) && mapping.range.is_some())
        .unwrap()
        .id;
    let offset = snapshot.visual.offset(space);
    let threshold = root.rect.y + offset;
    let child = snapshot
        .fragments
        .iter()
        .filter(|fragment| fragment.parent == Some(root.id) && fragment.node.is_some())
        .filter(|fragment| fragment.rect.y + fragment.rect.height > threshold)
        .filter(|fragment| matches!(fragment.content, domengine::Content::Box { .. }))
        .min_by_key(|fragment| (fragment.rect.y, fragment.id))
        .unwrap();
    (
        child.node.unwrap(),
        child.rect.y as i128 - root.rect.y as i128 - offset as i128,
    )
}

#[test]
fn lifecycle() {
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    page.load(&mut Source(content())).unwrap();
    let snapshot = page.inspect();
    let initial = snapshot.relevance;
    assert!(initial.estimated > 0);
    assert_eq!(initial.stale, 0);
    assert!(snapshot.reuse.estimation.resolved > 0);
    assert!(snapshot.reuse.estimation.error > 0);
    assert!(snapshot.reuse.estimation.maximum > 0);

    page.evaluate(
        "const control = scroll.attach(document.getElementById('list')); control.followEnd();",
    )
    .unwrap();
    page.render().unwrap();
    assert!(page.geometry("row19").unwrap().unwrap().height > 20);
    page.evaluate("control.dispose();").unwrap();
    let snapshot = page.inspect();
    let (space, _) = scroller(&mut page, "list");
    page.pan(snapshot.scene.unwrap(), space, -10000).unwrap();
    let top = page.inspect().relevance;
    page.evaluate("document.getElementById('row19').setText('changed content whose remembered geometry is now stale and must later be measured again');").unwrap();
    page.render().unwrap();
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content.contains("remembered geometry"))
    );
    let changed = page.inspect().relevance;
    assert!(changed.stale > 0, "top={top:?} changed={changed:?}");
    assert!(page.inspect().reuse.hits > 0);
    page.evaluate("scroll.attach(document.getElementById('list')).followEnd();")
        .unwrap();
    page.render().unwrap();
    let resolved = page.inspect();
    let finality = resolved.relevance;
    assert_eq!(finality.stale, 0, "{finality:?}");
    assert!(resolved.reuse.estimation.resolved > 0);
    assert!(resolved.reuse.estimation.error > 0);
}

#[test]
fn anchoring() {
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    page.load(&mut Source(content())).unwrap();
    page.evaluate(
        "const control = scroll.attach(document.getElementById('list')); control.followEnd();",
    )
    .unwrap();
    page.render().unwrap();
    image(&mut page);
    assert_eq!(
        page.wheel(10, 10, -80).unwrap(),
        webengine::Disposition::Applied
    );
    image(&mut page);

    let before = page.inspect();
    let original = anchor(&before, "list");
    let index = before
        .nodes
        .iter()
        .find(|node| node.id == original.0)
        .and_then(|node| node.attributes.get("id"))
        .and_then(|id| id.strip_prefix("row"))
        .and_then(|index| index.parse::<usize>().ok())
        .unwrap();
    assert!(index > 0);
    page.evaluate(&format!(
        "document.getElementById('row{}').setText('expanded content repeated across many wrapped lines to force a large measured geometry correction above the visible anchor');",
        index - 1
    ))
    .unwrap();
    page.render().unwrap();

    let after = page.inspect();
    assert_eq!(anchor(&after, "list"), original);
    let change = after.scrolling.last().unwrap();
    assert_eq!(change.cause, webengine::change::Cause::Anchor);
    assert_ne!(change.offset, change.previous);
    assert!(after.reuse.estimation.resolved > 0);
    assert!(after.reuse.estimation.error > 0);
}

#[test]
fn bounded() {
    let config = config::Store::default();
    config.set(
        "layout.estimates",
        config::Value::Object(
            [
                ("read".into(), true.into()),
                ("write".into(), true.into()),
                ("entries".into(), 2.into()),
                ("bytes".into(), 4096.into()),
            ]
            .into_iter()
            .collect(),
        ),
    );
    let mut page = Page::configured(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        config,
    )
    .unwrap();
    page.load(&mut Source(content())).unwrap();
    let estimation = page.inspect().reuse.estimation;
    assert!(estimation.entries <= 2);
    assert!(estimation.bytes <= 4096);
    assert!(estimation.evicted > 0);
    page.evaluate("scroll.attach(document.getElementById('list')).followEnd();")
        .unwrap();
    page.render().unwrap();
    assert!(page.geometry("row19").unwrap().unwrap().height > 20);
}
