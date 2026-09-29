use super::*;

#[test]
fn reconciliation() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    let scene = initial.scene.unwrap();
    page.pan(scene, inner, i64::MAX).unwrap();
    let inflight = page.frame().unwrap();
    assert_eq!(inflight.visual().offset(inner), 56);
    assert_eq!(
        page.inspect().scrolling.last().unwrap().cause,
        ScrollCause::Program
    );
    page.evaluate("const inner = document.getElementById('inner'); const c = document.getElementById('c'); inner.removeChild(c);").unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 16);
    assert_eq!(
        page.inspect().scrolling.last().unwrap().cause,
        ScrollCause::Layout
    );
    assert!(page.pan(scene, inner, -1).is_err());
    page.acknowledge(inflight.id()).unwrap();
    assert_eq!(page.inspect().shown.unwrap().offset(inner), 56);
    let reduced = accept(&mut page);
    assert_eq!(reduced.visual().offset(inner), 16);
    page.evaluate("document.getElementById('outer').removeChild(inner);")
        .unwrap();
    page.render().unwrap();
    let current = page.inspect();
    assert!(!current.visual.offsets.contains_key(&inner));
    assert!(page.pan(current.scene.unwrap(), inner, 1).is_err());
    let removed = accept(&mut page);
    assert!(removed.mappings().iter().all(|mapping| mapping.id != inner));
    assert!(reduced.mappings().iter().any(|mapping| mapping.id == inner));
}

#[test]
fn endanchoring() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    page.evaluate("scrolling.followEnd();").unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 56);
    assert_eq!(
        page.inspect().scrolling.last().unwrap().cause,
        ScrollCause::Policy
    );
    page.evaluate("const d = document.createElement('button'); d.setText('D'); document.getElementById('inner').appendChild(d);")
        .unwrap();
    page.render().unwrap();
    let followed = page.inspect();
    assert_eq!(space(&followed, "inner"), inner);
    assert_eq!(followed.visual.offset(inner), 96);
    page.wheel(40, 130, -20).unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 76);
    page.evaluate("const e = document.createElement('button'); e.setText('E'); document.getElementById('inner').appendChild(e);")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 76);
    page.evaluate("const inner = document.getElementById('inner'); const children = inner.children(); const first = document.createElement('button'); first.setText('First'); inner.appendChild(first); for (const child of children) { inner.appendChild(child); }")
        .unwrap();
    page.render().unwrap();
    let anchored = page.inspect();
    assert_eq!(anchored.visual.offset(inner), 116);
    assert_eq!(
        anchored.scrolling.last().unwrap().cause,
        ScrollCause::Anchor
    );
}

#[test]
fn anchorfallback() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    page.evaluate("scrolling.followEnd();").unwrap();
    page.render().unwrap();
    page.wheel(40, 130, -20).unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 36);
    page.evaluate("document.getElementById('before').setAttribute('style', 'height: 60px;');")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 36);
    page.evaluate("const inner = document.getElementById('inner'); inner.removeChild(document.getElementById('a')); const d = document.createElement('button'); d.setText('D'); inner.appendChild(d);")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 36);
}

#[test]
fn reconstruction() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    page.evaluate("let saved = null; document.getElementById('inner').addEventListener('scroll', (event) => { saved = event.scroll.position; }); scrolling.followEnd();")
        .unwrap();
    page.render().unwrap();
    page.wheel(40, 130, -20).unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 36);
    assert_eq!(
        page.evaluate("saved.target.getAttribute('id') + ':' + saved.inset;")
            .unwrap(),
        "a:-36"
    );
    assert_eq!(
        page.evaluate(
            "saved.fallbacks[0].target.getAttribute('id') + ':' + saved.fallbacks[0].inset;"
        )
        .unwrap(),
        "b:4"
    );
    page.evaluate("const host = document.getElementById('inner'); const key = saved.target.getAttribute('id'); const inset = saved.inset; for (const child of host.children()) { host.removeChild(child); } const pendingRows = []; for (const name of ['x', 'a', 'b', 'c']) { const child = document.createElement('button'); child.setAttribute('id', name); child.setText(name); pendingRows[pendingRows.length] = child; } host.appendChild(pendingRows[0]); scrolling.restore(pendingRows[1], inset);")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(inner), 0);
    page.evaluate("host.appendChild(pendingRows[1]); host.appendChild(pendingRows[2]); host.appendChild(pendingRows[3]);")
        .unwrap();
    page.render().unwrap();
    let restored = page.inspect();
    assert_eq!(restored.visual.offset(inner), 76);
    assert_eq!(
        restored.scrolling.last().unwrap().cause,
        ScrollCause::Restore
    );
}

#[test]
fn notifications() {
    let mut page = setup();
    page.evaluate("let observed = 0; let payload = ''; document.getElementById('inner').addEventListener('scroll', (event) => { observed = observed + 1; payload = event.scroll.previous + ':' + event.scroll.offset + ':' + event.scroll.range + ':' + event.scroll.edge + ':' + event.scroll.cause; bridge.postMessage('scroll'); });")
        .unwrap();
    page.render().unwrap();
    page.render().unwrap();
    assert_eq!(page.messages(), ["scroll"]);
    page.evaluate("observed = 0; payload = '';").unwrap();
    assert_eq!(page.wheel(40, 130, 8).unwrap(), Disposition::Applied);
    assert_eq!(page.wheel(40, 130, 8).unwrap(), Disposition::Applied);
    assert!(page.messages().is_empty());
    let pending = page.inspect();
    let change = pending.scrolling.last().unwrap();
    assert_eq!(change.previous, 0);
    assert_eq!(change.offset, 16);
    assert_eq!(change.cause, ScrollCause::User);
    assert_eq!(change.causes, [ScrollCause::User]);
    page.render().unwrap();
    assert_eq!(page.messages(), ["scroll"]);
    assert_eq!(page.evaluate("observed;").unwrap(), "1");
    assert_eq!(page.evaluate("payload;").unwrap(), "0:16:56:middle:user");
    page.render().unwrap();
    assert!(page.messages().is_empty());
}

#[test]
fn projection() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    let before = initial.scrollwork;
    let mapping = initial.mapping;
    page.pan(initial.scene.unwrap(), inner, 8).unwrap();
    let current = page.inspect();
    let moved = current.scrollwork;
    assert_eq!(moved.updates, before.updates + 1);
    assert_eq!(moved.incremental, before.incremental + 1);
    assert_eq!(moved.rebuilds, before.rebuilds);
    assert!(moved.spaces > before.spaces);
    assert!(moved.positions > before.positions);
    assert_eq!(current.mapping.updates, mapping.updates + 1);
    assert_eq!(current.mapping.revisions, mapping.revisions + 1);
    assert_eq!(current.mapping.spaces, mapping.spaces);
    assert_eq!(current.mapping.offsets, mapping.offsets + 1);
    assert!(current.mapping.affected > mapping.affected);
    page.evaluate("document.getElementById('a').setText('changed');")
        .unwrap();
    page.render().unwrap();
    let changed = page.inspect().scrollwork;
    assert_eq!(changed.updates, moved.updates + 1);
    assert_eq!(changed.rebuilds, moved.rebuilds + 1);
    assert_eq!(changed.incremental, moved.incremental);
    assert!(changed.elapsed >= moved.elapsed);
}

#[test]
fn materialization() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    page.pan(initial.scene.unwrap(), inner, 8).unwrap();
    let pending = page.inspect();
    assert_eq!(pending.materialization, initial.materialization);
    let frame = page.frame().unwrap();
    frame.mappings();
    let consumed = page
        .inspect()
        .materialization
        .since(pending.materialization);
    assert_eq!(consumed.materializations, 1);
    assert_eq!(consumed.spaces, frame.spaces() as u64);
    assert_eq!(consumed.queries + consumed.traversals, 1);
}

#[test]
fn replay() {
    let mut page = setup();
    page.render().unwrap();
    page.evaluate("let payload = ''; document.getElementById('inner').addEventListener('scroll', (event) => { payload = event.scroll.previous + ':' + event.scroll.offset + ':' + event.scroll.range + ':' + event.scroll.edge + ':' + event.scroll.causes[0]; bridge.postMessage('replay'); });")
        .unwrap();
    page.render().unwrap();
    assert!(page.messages().is_empty());
    page.render().unwrap();
    assert_eq!(page.messages(), ["replay"]);
    assert_eq!(page.evaluate("payload;").unwrap(), "0:0:56:start:layout");
}

#[test]
fn rangechange() {
    let mut page = setup();
    page.evaluate("let payload = ''; const inner = document.getElementById('inner'); inner.addEventListener('scroll', (event) => { payload = event.scroll.offset + ':' + event.scroll.range + ':' + event.scroll.edge + ':' + event.scroll.causes[0] + ':' + event.scroll.causes[1]; bridge.postMessage('change'); });")
        .unwrap();
    page.render().unwrap();
    page.render().unwrap();
    page.messages();
    page.evaluate(
        "const d = document.createElement('button'); d.setText('D'); inner.appendChild(d);",
    )
    .unwrap();
    page.render().unwrap();
    page.render().unwrap();
    assert_eq!(page.messages(), ["change"]);
    assert_eq!(
        page.evaluate("payload;").unwrap(),
        "0:96:start:layout:undefined"
    );
    page.evaluate(
        "const e = document.createElement('button'); e.setText('E'); inner.appendChild(e);",
    )
    .unwrap();
    page.render().unwrap();
    page.wheel(40, 130, 8).unwrap();
    page.render().unwrap();
    assert_eq!(page.messages(), ["change"]);
    assert_eq!(
        page.evaluate("payload;").unwrap(),
        "8:136:middle:layout:user"
    );
}

#[test]
fn order() {
    let mut page = setup();
    let current = page.inspect();
    let scene = current.scene.unwrap();
    let inner = space(&current, "inner");
    let outer = space(&current, "outer");
    page.evaluate("document.getElementById('inner').addEventListener('scroll', () => bridge.postMessage('inner')); document.getElementById('outer').addEventListener('scroll', () => bridge.postMessage('outer'));")
        .unwrap();
    page.pan(scene, outer, 8).unwrap();
    page.pan(scene, inner, 8).unwrap();
    page.pan(scene, outer, 8).unwrap();
    assert_eq!(
        page.inspect()
            .scrolling
            .iter()
            .rev()
            .take(2)
            .map(|change| change.space)
            .collect::<Vec<_>>(),
        [inner, outer]
    );
    page.render().unwrap();
    assert_eq!(page.messages(), ["outer", "inner"]);
}

#[test]
fn controllerlease() {
    let mut page = setup();
    assert!(
        page.evaluate("scroll.attach(document.getElementById('inner'));")
            .is_err()
    );
    page.evaluate("scrolling.dispose(); const replacement = scroll.attach(document.getElementById('inner')); replacement.followEnd();")
        .unwrap();
    page.render().unwrap();
    let current = page.inspect();
    assert_eq!(current.visual.offset(space(&current, "inner")), 56);
    assert!(page.evaluate("scrolling.followEnd();").is_err());
}
