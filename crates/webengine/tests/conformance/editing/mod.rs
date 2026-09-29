mod animation;
use super::{Extent, Page, loaded, source};
use webengine::Edit;
use webengine::ime::{Composition, Id, Input};
use webengine::keyboard::{self, Chord, Stroke};
use webengine::{Direction, Selection};

#[test]
fn selection() {
    let mut page = page();
    page.evaluate("a.setValue('A😀fi中'); a.setSelection(4, 1);")
        .unwrap();
    page.edit(&Edit::Insert("!".into())).unwrap();
    let snapshot = page.inspect();
    let editor = snapshot
        .nodes
        .iter()
        .find(|node| Some(node.id) == snapshot.focused)
        .unwrap()
        .editor
        .as_ref()
        .unwrap();
    assert_eq!(
        editor.selection,
        Selection {
            anchor: 4,
            focus: 1
        }
    );
    page.edit(&Edit::Insert("X".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "AX中");
    page.edit(&Edit::Move {
        direction: Direction::Start,
        extend: true,
    })
    .unwrap();
    page.edit(&Edit::Insert("replaced".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "replaced中");
    page.edit(&Edit::All).unwrap();
    page.edit(&Edit::Forward).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "");
    page.evaluate("a.setValue('same'); a.setSelection(4, 0); log = '';")
        .unwrap();
    page.edit(&Edit::Insert("same".into())).unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "before:same;");
    let snapshot = page.inspect();
    let editor = snapshot
        .nodes
        .iter()
        .find(|node| Some(node.id) == snapshot.focused)
        .unwrap()
        .editor
        .as_ref()
        .unwrap();
    assert_eq!(editor.selection, Selection::caret(4));
    assert!(
        page.edit(&Edit::Select(Selection {
            anchor: 5,
            focus: 0
        }))
        .is_err()
    );
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "same");
}

fn page() -> Page {
    let mut source = source();
    source.files.get_mut("/site/main.js").unwrap().body = "const body = document.getElementById('body'); const a = document.createElement('input'); a.setAttribute('id', 'a'); body.appendChild(a); const b = document.createElement('input'); body.appendChild(b); a.focus(); let log = ''; let previous = null; a.addEventListener('beforeinput', (event, context) => { log += 'before:' + a.getValue() + ';'; if (previous !== null) { previous.preventDefault(); } previous = context; if (event.data === '!') { context.preventDefault(); } }); a.addEventListener('input', (event) => { log += 'after:' + a.getValue() + ';'; });".into();
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
fn transaction() {
    let mut page = page();
    let before = page.inspect();
    page.edit(&Edit::Insert("A😀".into())).unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "before:;after:A😀;");
    assert_eq!(page.inspect().painted, before.painted);
    page.edit(&Edit::Insert("!".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "A😀");
    page.edit(&Edit::Backward).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "A");
    page.evaluate("log = ''; a.setValue('fi'); a.setCaret(1);")
        .unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "");
    page.edit(&Edit::Insert("x".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "fxi");
    page.render().unwrap();
    let snapshot = page.inspect();
    assert_eq!(snapshot.revision, snapshot.painted);
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.id == snapshot.focused.unwrap())
        .unwrap();
    assert_eq!(node.editor.as_ref().unwrap().text, "fxi");
    assert_eq!(node.editor.as_ref().unwrap().selection.focus, 2);
    page.evaluate("log = ''; a.setValue('');").unwrap();
    page.edit(&Edit::Backward).unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "before:;");
    assert!(page.edit(&Edit::Insert("\n".into())).is_err());
    assert_eq!(page.evaluate("log;").unwrap(), "before:;");
    page.close();
    assert_eq!(page.inspect().focused, None);
    assert!(page.edit(&Edit::Insert("late".into())).is_err());
}

#[test]
fn composition() {
    let mut page = page();
    page.evaluate("a.setValue('A中'); a.setSelection(1, 2); log = ''; a.addEventListener('input', (event) => { log += event.inputType + ':' + event.data + ':' + a.getValue(); });").unwrap();
    let id = Id(11);
    assert!(
        page.composition(&Input::Update(Composition {
            id,
            replacement: Selection {
                anchor: 1,
                focus: 2
            },
            text: "zhong".into(),
            selection: Selection::caret(5),
            spans: Vec::new(),
        }))
        .unwrap()
    );
    let snapshot = page.inspect();
    let editor = snapshot
        .nodes
        .iter()
        .find(|node| Some(node.id) == snapshot.focused)
        .unwrap()
        .editor
        .as_ref()
        .unwrap();
    assert_eq!(editor.text, "A中");
    assert_eq!(editor.composition.as_ref().unwrap().text, "zhong");
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "A中");
    assert!(
        page.composition(&Input::Commit {
            id,
            text: "钟".into(),
        })
        .unwrap()
    );
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "A钟");
    assert_eq!(
        page.evaluate("log;").unwrap(),
        "after:A钟;insertCompositionText:钟:A钟"
    );

    page.composition(&Input::Update(Composition {
        id: Id(12),
        replacement: Selection::caret(2),
        text: "stale".into(),
        selection: Selection::caret(5),
        spans: Vec::new(),
    }))
    .unwrap();
    page.evaluate("b.focus();").unwrap();
    assert!(
        !page
            .composition(&Input::Commit {
                id: Id(12),
                text: "lost".into(),
            })
            .unwrap()
    );
    assert_eq!(
        page.evaluate("a.getValue() + b.getValue();").unwrap(),
        "A钟"
    );
}

#[test]
fn retargeting() {
    let mut page = page();
    page.evaluate("a.addEventListener('beforeinput', (event) => { b.focus(); });")
        .unwrap();
    page.edit(&Edit::Insert("lost".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue() + b.getValue();").unwrap(), "");
    page.edit(&Edit::Insert("second".into())).unwrap();
    assert_eq!(page.evaluate("b.getValue();").unwrap(), "second");
    page.evaluate("body.removeChild(b);").unwrap();
    assert_eq!(page.inspect().focused, None);
    page.edit(&Edit::Insert("detached".into())).unwrap();
    assert_eq!(page.evaluate("b.getValue();").unwrap(), "second");
    page.edit(&Edit::Next).unwrap();
    assert_eq!(
        page.evaluate("document.focused() === document.getElementById('button');")
            .unwrap(),
        "true"
    );
    page.edit(&Edit::Next).unwrap();
    assert_eq!(page.evaluate("document.focused() === a;").unwrap(), "true");
    page.edit(&Edit::Blur).unwrap();
    assert_eq!(page.evaluate("document.focused();").unwrap(), "null");
}

#[test]
fn focusing() {
    let mut page = page();
    page.evaluate("log = ''; let aBlur = 0; let bFocus = 0; let bBlur = 0; a.addEventListener('blur', () => { aBlur += 1; log += 'a:' + (document.focused() === b) + ';'; }); b.addEventListener('focus', () => { bFocus += 1; log += 'b:' + (document.focused() === b) + ';'; }); b.addEventListener('blur', () => { bBlur += 1; log += 'blur:' + (document.focused() === null) + ';'; }); body.addEventListener('focus', () => { log += 'bubble;'; }); b.focus(); b.focus();").unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "a:true;b:true;");
    assert_eq!(page.evaluate("aBlur + ':' + bFocus;").unwrap(), "1:1");
    page.edit(&Edit::Blur).unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "a:true;b:true;blur:true;");

    page.evaluate("a.focus(); log = ''; bFocus = 0; a.addEventListener('blur', () => { document.getElementById('button').focus(); }); b.focus();").unwrap();
    assert_eq!(page.evaluate("bFocus;").unwrap(), "0");
    assert_eq!(
        page.evaluate("document.focused() === document.getElementById('button');")
            .unwrap(),
        "true"
    );

    page.evaluate("b.focus(); log = ''; body.removeChild(b);")
        .unwrap();
    assert_eq!(page.evaluate("bBlur;").unwrap(), "3");
    assert_eq!(page.evaluate("document.focused();").unwrap(), "null");
}

#[test]
fn activation() {
    let mut source = source();
    let mut page = loaded(&mut source);
    page.evaluate("button.focus();").unwrap();
    assert_eq!(
        page.evaluate("document.focused() === button;").unwrap(),
        "true"
    );
    page.input(&keyboard::Input::Key(Stroke {
        chord: Chord::parse("Enter").unwrap(),
        repeat: false,
        text: String::new(),
    }))
    .unwrap();
    assert_eq!(page.evaluate("count;").unwrap(), "1");
    page.edit(&Edit::Next).unwrap();
    assert_eq!(
        page.evaluate("document.focused() === button;").unwrap(),
        "true"
    );
}

#[test]
fn transition() {
    let mut source = source();
    let mut page = loaded(&mut source);
    page.evaluate("let pointerLog = ''; const body = document.getElementById('body'); body.addEventListener('pointerenter', () => { pointerLog += 'body-enter;'; }); body.addEventListener('pointerleave', () => { pointerLog += 'body-leave;'; }); button.addEventListener('pointerenter', () => { pointerLog += 'button-enter;'; }); button.addEventListener('pointerleave', () => { pointerLog += 'button-leave;'; });").unwrap();
    let button = page.hit(50, 30);
    page.transition(None, Some(&button)).unwrap();
    page.transition(Some(&button), None).unwrap();
    assert_eq!(
        page.evaluate("pointerLog;").unwrap(),
        "body-enter;button-enter;button-leave;body-leave;"
    );
}

#[test]
fn failure() {
    let mut page = page();
    page.evaluate(
        "a.addEventListener('beforeinput', () => { a.setValue('script'); throw Error('stop'); });",
    )
    .unwrap();
    page.edit(&Edit::Insert("user".into())).unwrap();
    assert_eq!(page.evaluate("a.getValue();").unwrap(), "scriptuser");
    page.evaluate("b.focus(); b.addEventListener('input', () => { throw Error('after'); });")
        .unwrap();
    let before = page.inspect().painted;
    page.edit(&Edit::Insert("retained".into())).unwrap();
    assert_eq!(page.evaluate("b.getValue();").unwrap(), "retained");
    assert_eq!(page.inspect().painted, before);
    page.render().unwrap();
    assert_eq!(page.inspect().painted, page.inspect().revision);
}
