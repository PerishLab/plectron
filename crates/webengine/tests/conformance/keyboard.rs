use super::{Extent, Page, source};
use webengine::keyboard::{Chord, Input, Outcome, Stroke};

fn page() -> Page {
    let mut source = source();
    source.files.get_mut("/site/main.js").unwrap().body = "const body = document.getElementById('body'); const a = document.createElement('input'); body.appendChild(a); const b = document.createElement('input'); body.appendChild(b); a.focus(); let count = 0; a.addEventListener('commit', () => { count += 1; });".into();
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
fn key(chord: &str, text: &str, repeat: bool) -> Input {
    Input::Key(Stroke {
        chord: Chord::parse(chord).unwrap(),
        text: text.into(),
        repeat,
    })
}
#[test]
fn lifecycle() {
    let mut page = page();
    page.evaluate("const group = shortcuts.register('editor', 0, [{chord: 'Primary+KeyK', command: 'selectAll'}]); a.setValue('first');").unwrap();
    assert!(
        page.input(&key("Primary+KeyK", "k", false))
            .unwrap()
            .consumed
    );
    page.input(&Input::Text("replaced".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "replaced");
    page.input(&key("Escape", "", false)).unwrap();
    assert_eq!(
        page.input(&key("Primary+KeyK", "k", false)).unwrap(),
        Outcome::default()
    );
    page.evaluate("b.focus(); b.setValue('second');").unwrap();
    assert!(
        page.input(&key("Primary+KeyK", "k", false))
            .unwrap()
            .consumed
    );
    page.input(&Input::Text("other".into())).unwrap();
    assert_eq!(page.evaluate("b.getValue();").unwrap(), "other");
    assert!(
        page.evaluate("group.replace(1, [{chord: 'bad', command: 'commit'}]);")
            .is_err()
    );
    assert!(
        page.input(&key("Primary+KeyK", "k", false))
            .unwrap()
            .consumed
    );
    page.evaluate("group.replace(1, []);").unwrap();
    assert!(
        !page
            .input(&key("Primary+KeyK", "k", false))
            .unwrap()
            .handled
    );
    assert_eq!(page.evaluate("b.getValue();").unwrap(), "k");
    page.evaluate(
        "group.replace(1, [{chord: 'Primary+KeyK', command: 'selectAll'}]); group.dispose();",
    )
    .unwrap();
    assert!(page.evaluate("group.dispose();").is_err());
    assert!(page.evaluate("group.replace(1, []);").is_err());
    let mut other = self::page();
    assert!(
        !other
            .input(&key("Primary+KeyK", "k", false))
            .unwrap()
            .handled
    );
    page.close();
    assert!(page.input(&key("Primary+KeyK", "k", false)).is_err());
}
#[test]
fn dispatch() {
    let mut page = page();
    page.evaluate("const group = shortcuts.register('editor', 0, [{chord: 'Enter', command: 'selectAll'}]); const low = shortcuts.register('page', 999, [{chord: 'Enter', command: 'blur'}]); a.setValue('draft');").unwrap();
    page.input(&key("Enter", "", false)).unwrap();
    assert_eq!(page.evaluate("count;").unwrap(), "0");
    assert_eq!(page.evaluate("document.focused() === a;").unwrap(), "true");
    page.evaluate("group.dispose(); low.dispose();").unwrap();
    page.input(&key("Enter", "", false)).unwrap();
    page.input(&key("Enter", "", true)).unwrap();
    assert_eq!(page.evaluate("count;").unwrap(), "1");
    page.input(&key("ArrowLeft", "", false)).unwrap();
    page.input(&key("ArrowRight", "", true)).unwrap();
    page.input(&key("Shift+ArrowRight", "", true)).unwrap();
    page.input(&Input::Text("X".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "dXaft");
    page.evaluate("const pass = shortcuts.register('editor', 0, [{chord: 'KeyA', command: 'selectAll', consume: false}]);").unwrap();
    assert_eq!(
        page.input(&key("KeyA", "字", false)).unwrap(),
        Outcome {
            handled: true,
            consumed: false
        }
    );
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "字");
    page.evaluate("pass.dispose(); const high = shortcuts.register('page', 1, [{chord: 'Tab', command: 'commit'}]);").unwrap();
    page.input(&key("Escape", "", false)).unwrap();
    assert!(page.input(&key("Tab", "", false)).unwrap().consumed);
    assert_eq!(
        page.evaluate("document.focused() === document.getElementById('button');")
            .unwrap(),
        "true"
    );
    page.input(&key("Alt+ArrowLeft", "", false)).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "字");
}
#[test]
fn cancellation() {
    let mut page = page();
    page.evaluate("a.setValue('kept'); const group = shortcuts.register('editor', 0, [{chord: 'KeyX', command: 'backward'}]); a.addEventListener('beforeinput', (event, context) => { context.preventDefault(); group.dispose(); });").unwrap();
    assert!(page.input(&key("KeyX", "x", false)).unwrap().consumed);
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "kept");
    assert!(page.input(&key("KeyX", "x", false)).is_err());
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "kept");
    assert!(page.input(&Input::Text("bad\n".into())).is_err());
}
