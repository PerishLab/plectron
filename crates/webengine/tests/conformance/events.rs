#[path = "events/config.rs"]
mod config;
use super::{Extent, Page, source};
use webengine::{Edit, Effect, Origin, Propagation};

fn page() -> Page {
    let mut source = source();
    source.files.get_mut("/site/main.js").unwrap().body = "const body = document.getElementById('body'); const parent = document.createElement('div'); const child = document.createElement('input'); body.appendChild(parent); parent.appendChild(child); let log = '';".into();
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 200,
            height: 120,
        },
    )
    .unwrap();
    page.load(&mut source).unwrap();
    page
}
#[test]
fn propagation() {
    let mut page = page();
    page.evaluate("body.addEventListener('test', (event, context) => { log += 'capture;'; parent.removeChild(child); }, {capture: true}); parent.addEventListener('test', (event, context) => { log += context.phase + ';'; }); child.addEventListener('test', (event, context) => { log += context.phase + ':' + (event.target === child) + ':' + (context.currentTarget === child) + ';'; }); child.dispatchEvent('test', 'data', {bubbles: true});").unwrap();
    assert_eq!(
        page.evaluate("log;").unwrap(),
        "capture;target:true:true;bubble;"
    );
    let snapshot = page.inspect();
    let record = snapshot.events.last().unwrap();
    assert_eq!(record.origin, Origin::Script);
    assert_eq!(record.deliveries.len(), 3);
    assert_eq!(record.deliveries[0].phase, Propagation::Capture);
    assert_eq!(record.deliveries[2].phase, Propagation::Bubble);
    page.evaluate("log = ''; child.dispatchEvent('test', 'detached', {bubbles: true});")
        .unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "target:true:true;");
    page.evaluate("child.addEventListener('stop', (event, context) => { log += 'a'; context.stopPropagation(); }, {capture: true}); child.addEventListener('stop', () => { log += 'b'; }); parent.appendChild(child); parent.addEventListener('stop', () => { log += 'wrong'; }); log = ''; child.dispatchEvent('stop', '', {bubbles: true});").unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "ab");
    page.evaluate("child.addEventListener('immediate', (event, context) => { log += 'a'; context.stopImmediatePropagation(); }, {capture: true}); child.addEventListener('immediate', () => { log += 'wrong'; }); log = ''; child.dispatchEvent('immediate', '', {bubbles: true});").unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "a");
}
#[test]
fn lifecycle() {
    let mut page = page();
    page.evaluate("const scope = events.scope(); let second = null; child.addEventListener('test', () => { log += 'a'; second.dispose(); child.addEventListener('test', () => { log += 'new'; }, {scope: scope}); }, {once: true}); second = child.addEventListener('test', () => { log += 'wrong'; }); child.addEventListener('test', () => { log += 'b'; }, {scope: scope}); child.dispatchEvent('test', '');").unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "ab");
    page.evaluate("log = ''; child.dispatchEvent('test', '');")
        .unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "bnew");
    page.evaluate("scope.dispose(); log = ''; child.dispatchEvent('test', '');")
        .unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "");
    assert!(page.evaluate("second.dispose();").is_err());
    assert!(
        page.evaluate("child.addEventListener('test', () => {}, {scope: scope});")
            .is_err()
    );
    assert!(
        page.evaluate(
            "child.addEventListener('test', () => {}, {scope: {dispose: scope.dispose}});"
        )
        .is_err()
    );
    page.evaluate("const shared = events.scope(); child.addEventListener('group', () => { shared.dispose(); }, {scope: shared}); child.addEventListener('group', () => { log += 'wrong'; }, {scope: shared}); child.dispatchEvent('group', '');").unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "");
}
#[test]
fn reentry() {
    let mut page = page();
    page.evaluate("let outer = null; let saved = null; child.addEventListener('inner', (event, context) => { log += outer.preventDefault() + ';'; saved = context; }); child.addEventListener('outer', (event, context) => { outer = context; child.dispatchEvent('inner', '', {cancelable: true}); log += saved.preventDefault() + ';'; log += context.preventDefault() + ';'; }); const result = child.dispatchEvent('outer', '', {cancelable: true});").unwrap();
    assert_eq!(
        page.evaluate("log + result.cancelled + ';' + outer.preventDefault();")
            .unwrap(),
        "false;false;true;true;false"
    );
    let records = page.inspect().events;
    assert_eq!(records[0].parent, Some(records[1].id));
    assert_eq!(records[0].cancelled, None);
    assert!(records[1].cancelled.is_some());
    page.evaluate("let count = 0; child.addEventListener('once', () => { count += 1; child.dispatchEvent('once', ''); }, {once: true}); child.dispatchEvent('once', '');").unwrap();
    assert_eq!(page.evaluate("count;").unwrap(), "1");
    page.evaluate("let previous = null; child.addEventListener('lease', (event, context) => { previous = context; }); child.addEventListener('lease', () => { log = previous.preventDefault(); }); child.dispatchEvent('lease', '', {cancelable: true});").unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "false");
}
#[test]
fn outcomes() {
    let mut page = page();
    page.evaluate("child.focus(); child.addEventListener('beforeinput', (event, context) => { log += 'observe;'; if (context.preventDefault !== undefined) { throw Error('authority leaked'); } }, {observe: true}); const throwing = child.addEventListener('beforeinput', () => { throw Error('listener'); }); child.addEventListener('input', () => { log += 'input;'; });").unwrap();
    page.edit(&Edit::Insert("text".into())).unwrap();
    assert_eq!(page.evaluate("child.getValue();").unwrap(), "text");
    assert_eq!(page.evaluate("log;").unwrap(), "observe;input;");
    let snapshot = page.inspect();
    let before = snapshot
        .events
        .iter()
        .find(|record| record.kind == "beforeinput")
        .unwrap();
    assert_eq!(before.action, Effect::Applied);
    assert!(before.failure.is_none());
    assert!(
        before.deliveries[1]
            .error
            .as_ref()
            .unwrap()
            .contains("listener")
    );
    page.evaluate("const cancel = child.addEventListener('beforeinput', (event, context) => { context.preventDefault(); });").unwrap();
    page.edit(&Edit::Insert("ignored".into())).unwrap();
    assert_eq!(
        page.inspect().events.last().unwrap().action,
        Effect::Prevented
    );
    assert_eq!(page.evaluate("child.getValue();").unwrap(), "text");
    page.evaluate("cancel.dispose(); throwing.dispose(); child.addEventListener('beforeinput', () => { child.setValue('kept'); bridge.postMessage(1); });").unwrap();
    assert!(page.edit(&Edit::Insert("ignored".into())).is_err());
    assert_eq!(page.evaluate("child.getValue();").unwrap(), "kept");
    assert_eq!(page.inspect().events.last().unwrap().action, Effect::Failed);
    page.evaluate("child.dispatchEvent('input', 'synthetic');")
        .unwrap();
    assert_eq!(
        page.inspect().events.last().unwrap().action,
        Effect::Notification
    );
}
#[test]
fn boundaries() {
    let mut page = page();
    page.evaluate("child.focus(); child.setValue('kept'); let commits = 0; child.addEventListener('commit', () => { commits += 1; }); child.dispatchEvent('beforeinput', 'replace', {cancelable: true}); child.dispatchEvent('click', '');").unwrap();
    assert_eq!(
        page.evaluate("child.getValue() + ':' + commits;").unwrap(),
        "kept:0"
    );
    page.evaluate(
        "const loop = child.addEventListener('loop', () => { child.dispatchEvent('loop', ''); });",
    )
    .unwrap();
    assert!(page.evaluate("child.dispatchEvent('loop', '');").is_err());
    assert!(
        page.inspect()
            .events
            .iter()
            .any(|record| record.failure.is_some())
    );
    page.evaluate("loop.dispose(); child.dispatchEvent('fine', '');")
        .unwrap();
    assert_eq!(page.inspect().events.last().unwrap().parent, None);
    for _ in 0..40 {
        page.evaluate("child.dispatchEvent('bounded', '');")
            .unwrap();
    }
    assert_eq!(page.inspect().events.len(), 32);
    let last = page.inspect().events.last().unwrap().id;
    page.close();
    assert!(page.evaluate("child.dispatchEvent('late', '');").is_err());
    assert_eq!(page.inspect().events.last().unwrap().id, last);
}

#[test]
fn truncation() {
    let mut page = page();
    for _ in 0..64 {
        page.evaluate("body.addEventListener('large', () => {}, {capture: true});")
            .unwrap();
    }
    page.evaluate("child.addEventListener('large', () => { throw Error('late failure'); }); child.dispatchEvent('large', '');").unwrap();
    let snapshot = page.inspect();
    let record = snapshot.events.last().unwrap();
    assert_eq!(record.deliveries.len(), 64);
    assert_eq!(record.omitted, 1);
    assert_eq!(record.errors, 1);
    assert!(record.exception.as_ref().unwrap().contains("late failure"));
    assert!(
        page.evaluate("body.addEventListener('large', () => {});")
            .is_err()
    );
    page.evaluate("child.focus();").unwrap();
    page.edit(&Edit::Insert("a".repeat(4096))).unwrap();
    assert!(page.edit(&Edit::Insert("overflow".into())).is_err());
    let snapshot = page.inspect();
    let record = snapshot.events.last().unwrap();
    assert_eq!(record.action, Effect::Failed);
    assert!(record.failure.is_some());
}
