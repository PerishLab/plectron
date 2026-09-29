use super::{loaded, source};
mod history;
mod retention;
mod wrappers;

#[test]
fn publication() {
    let mut page = loaded(&mut source());
    page.evaluate("const calculate = (x, y) => x * y + 2; button.addEventListener('click', () => { button.setText('JIT:' + calculate(3, 4)); });").unwrap();
    for _ in 0..40 {
        page.click(50, 30).unwrap();
        page.messages();
    }
    page.render().unwrap();
    let snapshot = page.inspect();
    assert!(snapshot.nodes.iter().any(|node| node.content == "JIT:14"));
    assert!(snapshot.activity.calls > 0);
    assert!(snapshot.activity.natives > 0);
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    assert!(
        snapshot
            .performance
            .functions
            .iter()
            .any(|function| function.native == 9)
    );
    assert!(
        snapshot
            .performance
            .functions
            .iter()
            .any(|function| function.rejection.is_some())
    );
    let other = loaded(&mut source()).inspect();
    assert!(
        other
            .performance
            .functions
            .iter()
            .all(|function| function.native != 9 && function.rejection.is_none())
    );
    page.close();
    assert_eq!(
        page.inspect().performance.functions.len(),
        snapshot.performance.functions.len()
    );
}

#[test]
fn geometry() {
    use graphics::shared::{Backend, Cpu};
    let mut page = loaded(&mut source());
    let original = page.inspect().reuse;
    page.evaluate("button.setAttribute('style', 'background-color: #aa0000;');")
        .unwrap();
    page.render().unwrap();
    let red = page.frame().unwrap();
    let before = Cpu.render(&red).unwrap();
    page.acknowledge(red.id()).unwrap();
    assert_eq!(page.inspect().reuse.computed, 0);
    assert_eq!(page.inspect().reuse.charged, original.charged);
    page.evaluate("button.setAttribute('style', 'background-color: #0000aa;');")
        .unwrap();
    page.render().unwrap();
    let blue = page.frame().unwrap();
    assert_ne!(Cpu.render(&blue).unwrap(), before);
    assert_eq!(Cpu.render(&red).unwrap(), before);
    page.acknowledge(blue.id()).unwrap();
    let painted = page.inspect().painted;
    page.evaluate("document.getElementById('body').setAttribute('style', 'overflow: auto;');")
        .unwrap();
    assert!(page.render().is_err());
    let failed = page.inspect();
    assert_eq!(failed.painted, painted);
    assert!(failed.reuse.failure.is_some());
    assert_eq!(failed.revision, Some(failed.reuse.revision));
    page.evaluate(
        "document.getElementById('body').setAttribute('style', 'height: 100px; overflow: auto;');",
    )
    .unwrap();
    page.render().unwrap();
    let snapshot = page.inspect();
    assert_eq!(snapshot.painted, snapshot.revision);
    assert_eq!(snapshot.reuse.revision, snapshot.revision.unwrap());
    assert!(snapshot.reuse.failure.is_none());
    page.close();
    assert_eq!(page.inspect().reuse.revision, snapshot.reuse.revision);
}

#[test]
fn caches() {
    use shared::cache::{Command, Policy};
    let mut page = loaded(&mut source());
    let before = page.inspect();
    for name in ["paint", "fragments", "subsequences"] {
        assert!(before.caches.iter().any(|cache| cache.name == name));
    }
    assert!(page.cache(u64::MAX, Command::Clear).is_err());
    let disabled = Policy {
        read: false,
        write: false,
        entries: 512,
        bytes: 4_194_304,
    };
    for cache in &before.caches {
        page.cache(cache.id, Command::Configure { policy: disabled })
            .unwrap();
    }
    assert_eq!(page.inspect().scene, before.scene);
    page.evaluate("button.setText('Count');").unwrap();
    page.render().unwrap();
    let disabled = page.inspect();
    assert_eq!(disabled.reuse.hits, 0);
    assert_eq!(disabled.reuse.charged, before.reuse.charged);
    assert!(
        disabled
            .caches
            .iter()
            .all(|cache| cache.requested == cache.applied)
    );
    for cache in &before.caches {
        page.cache(cache.id, Command::Destroy).unwrap();
    }
    assert!(
        page.inspect()
            .caches
            .iter()
            .all(|cache| cache.closed && cache.stats.bytes == 0)
    );
    page.evaluate("button.setText('After');").unwrap();
    page.render().unwrap();
    let finality = page.inspect();
    assert_eq!(finality.reuse.hits, 0);
    assert!(finality.nodes.iter().any(|node| node.content == "After"));
    page.close();
    assert_eq!(page.inspect().caches, finality.caches);
}

#[test]
fn incremental() {
    use graphics::shared::{Backend, Cpu};
    let mut page = loaded(&mut source());
    let before = page.inspect();
    page.evaluate("button.setText('First'); button.setText('Final');")
        .unwrap();
    page.render().unwrap();
    let updated = page.inspect();
    assert_eq!(updated.update.committed, updated.revision);
    assert!(updated.update.rebuilt < updated.update.nodes);
    assert_eq!(updated.update.changes, 4);
    let frame = page.frame().unwrap();
    let pixels = Cpu.render(&frame).unwrap();
    page.acknowledge(frame.id()).unwrap();
    page.evaluate("button.setAttribute('style', 'overflow: auto; height: auto;');")
        .unwrap();
    assert!(page.render().is_err());
    let failed = page.inspect();
    assert_eq!(failed.update.committed, updated.revision);
    assert_eq!(failed.painted, updated.painted);
    assert!(failed.update.failure.is_some());
    page.evaluate("button.setAttribute('style', 'height: 40px;');")
        .unwrap();
    page.render().unwrap();
    let recovered = page.inspect();
    assert_eq!(recovered.update.committed, recovered.revision);
    assert_eq!(recovered.update.changes, 2);
    assert!(recovered.update.failure.is_none());
    assert_eq!(Cpu.render(&frame).unwrap(), pixels);
    assert!(recovered.revision > before.revision);
    page.close();
    assert_eq!(page.inspect().update.committed, recovered.update.committed);
}

fn records(items: &[graphics::shared::Item]) -> usize {
    items
        .iter()
        .map(|item| match item {
            graphics::shared::Item::Draw { .. } => 1,
            graphics::shared::Item::Group { items, .. } => records(items),
            _ => panic!("snapshot must contain the actual retained scene"),
        })
        .sum()
}

#[test]
fn painting() {
    use graphics::shared::{Backend, Cpu};
    use shared::cache::Command;
    let mut page = loaded(&mut source());
    page.evaluate("button.setText('Record');").unwrap();
    page.render().unwrap();
    let initial = page.inspect();
    assert_eq!(records(&initial.items), initial.painting.requests);
    let old = page.frame().unwrap();
    let pixels = Cpu.render(&old).unwrap();
    page.acknowledge(old.id()).unwrap();
    page.evaluate("button.setAttribute('style', 'translate: 8px 2px; opacity: 0.5;');")
        .unwrap();
    page.render().unwrap();
    let moved = page.inspect();
    assert_eq!(Some(moved.painting.revision), moved.revision);
    assert_eq!(moved.painting.recorded, 0);
    assert_eq!(moved.painting.hits, moved.painting.requests);
    let cache = moved
        .caches
        .iter()
        .find(|cache| cache.name == "paint")
        .unwrap();
    page.cache(cache.id, Command::Destroy).unwrap();
    assert_eq!(page.inspect().scene, moved.scene);
    page.evaluate("button.setAttribute('style', 'translate: 16px 2px; opacity: 0.5;');")
        .unwrap();
    page.render().unwrap();
    let fresh = page.inspect();
    assert_eq!(fresh.painting.hits, 0);
    assert_eq!(fresh.painting.recorded, fresh.painting.requests);
    assert!(fresh.painting.failure.is_none());
    page.close();
    assert_eq!(Cpu.render(&old).unwrap(), pixels);
    assert_eq!(page.inspect().painting.revision, fresh.painting.revision);
}
