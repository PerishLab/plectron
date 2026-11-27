use super::super::{File, Source};
use domengine::Extent;
use graphics::shared::{Backend, Cpu};
use std::sync::{Arc, Mutex};
use webengine::{
    Edit, Fault, Page, Selection, ime,
    keyboard::{Chord, Clipboard, Input, Port, Stroke},
};

fn key(chord: &str) -> Input {
    Input::Key(Stroke {
        chord: Chord::parse(chord).unwrap(),
        repeat: false,
        text: String::new(),
    })
}

fn page() -> Page {
    let mut source = Source {
        requested: Vec::new(),
        files: [
            (
                "/site/index.html".into(),
                File {
                    mime: "text/html".into(),
                    body: "<html><head><link rel='stylesheet' href='./style.css'></head><body><p-area id='multi' style='width: 140px; height: 80px;'></p-area><input id='single' style='width: 140px; height: 24px;'></body></html>".into(),
                },
            ),
            (
                "/site/style.css".into(),
                File {
                    mime: "text/css".into(),
                    body: "p-area {overflow: hidden; overflow-wrap: anywhere;} p-caret {opacity: 1;}".into(),
                },
            ),
        ]
        .into_iter()
        .collect(),
    };
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 200,
            height: 160,
        },
    )
    .unwrap();
    page.load(&mut source).unwrap();
    page.evaluate(&fixtures::script(
        "document",
        "const multi = document.getElementById('multi'); const text = document.createTextNode(''); const caret = document.createElement('p-caret'); multi.appendChild(text); multi.appendChild(caret); document.attachEditor(multi, text, caret, true); const single = document.getElementById('single'); let log = ''; let commits = 0; let block = false; multi.addEventListener('beforeinput', (event, context) => { log += 'before:' + event.inputType + ';'; if (block) { context.preventDefault(); } }); multi.addEventListener('input', (event) => { log += 'after:' + event.inputType + ';'; }); multi.addEventListener('commit', () => { commits += 1; }); multi.focus();",
    ))
    .unwrap();
    page
}

#[test]
fn editing() {
    let mut page = page();
    page.input(&Input::Text("abcdef".into())).unwrap();
    page.input(&key("Shift+Enter")).unwrap();
    page.input(&Input::Text("xy".into())).unwrap();
    page.input(&key("Shift+Enter")).unwrap();
    page.input(&Input::Text("123456".into())).unwrap();
    assert_eq!(
        page.evaluate("multi.getValue();").unwrap(),
        "abcdef\nxy\n123456"
    );
    assert!(
        page.evaluate("log;")
            .unwrap()
            .contains("before:insertLineBreak;after:insertLineBreak;")
    );
    page.input(&key("Enter")).unwrap();
    assert_eq!(page.evaluate("commits;").unwrap(), "1");
    page.evaluate("multi.setCaret(5);").unwrap();
    page.input(&key("ArrowDown")).unwrap();
    page.input(&key("ArrowDown")).unwrap();
    let snapshot = page.inspect();
    let editor = snapshot
        .nodes
        .iter()
        .find(|node| Some(node.id) == snapshot.focused)
        .unwrap()
        .editor
        .as_ref()
        .unwrap();
    assert_eq!(editor.selection, Selection::caret(15));
    page.input(&key("Shift+ArrowUp")).unwrap();
    page.input(&key("Shift+ArrowUp")).unwrap();
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
            anchor: 15,
            focus: 5
        }
    );
    page.render().unwrap();
    let frame = page.frame().unwrap();
    Cpu.render(&frame).unwrap();
    page.acknowledge(frame.id()).unwrap();

    let large = "a".repeat(5000);
    page.edit(&Edit::All).unwrap();
    page.edit(&Edit::Insert(large)).unwrap();
    page.evaluate("multi.setCaret(4500); multi.setSelection(10, 4500);")
        .unwrap();
    page.evaluate("single.focus();").unwrap();
    page.input(&key("Shift+Enter")).unwrap();
    assert_eq!(page.evaluate("single.getValue();").unwrap(), "");
    assert!(page.evaluate("single.setCaret(4097);").is_err());
}

#[test]
fn clipboard() {
    let mut page = page();
    page.evaluate("multi.setValue('first\\nsecond'); multi.setSelection(2, 9); log = '';")
        .unwrap();
    page.input(&key("Primary+KeyC")).unwrap();
    assert_eq!(page.evaluate("log;").unwrap(), "");
    assert_eq!(page.evaluate("multi.getValue();").unwrap(), "first\nsecond");

    page.evaluate("multi.setCaret(12);").unwrap();
    page.input(&key("Primary+KeyV")).unwrap();
    assert_eq!(
        page.evaluate("multi.getValue();").unwrap(),
        "first\nsecondrst\nsec"
    );
    assert_eq!(
        page.evaluate("log;").unwrap(),
        "before:insertFromPaste;after:insertFromPaste;"
    );

    page.evaluate("single.focus(); single.setValue('ok'); single.setCaret(2);")
        .unwrap();
    page.input(&key("Primary+KeyV")).unwrap();
    assert_eq!(page.evaluate("single.getValue();").unwrap(), "okrstsec");

    page.evaluate("multi.focus(); multi.setValue('alpha\\nbeta'); multi.setSelection(0, 6); log = ''; block = true;")
        .unwrap();
    page.input(&key("Primary+KeyX")).unwrap();
    assert_eq!(page.evaluate("multi.getValue();").unwrap(), "alpha\nbeta");
    assert_eq!(page.evaluate("log;").unwrap(), "before:deleteByCut;");

    page.evaluate("block = false; log = '';").unwrap();
    page.input(&key("Primary+KeyX")).unwrap();
    assert_eq!(page.evaluate("multi.getValue();").unwrap(), "beta");
    assert_eq!(
        page.evaluate("log;").unwrap(),
        "before:deleteByCut;after:deleteByCut;"
    );

    page.input(&key("Primary+KeyV")).unwrap();
    assert_eq!(page.evaluate("multi.getValue();").unwrap(), "alpha\nbeta");

    page.edit(&Edit::Paste(Some("!\r\n\0?".into()))).unwrap();
    assert_eq!(
        page.evaluate("multi.getValue();").unwrap(),
        "alpha\n!\n?beta"
    );

    struct Probe(Mutex<Option<String>>);
    impl Port for Probe {
        fn read(&self) -> Result<Option<String>, Fault> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn write(&self, text: &str) -> Result<(), Fault> {
            *self.0.lock().unwrap() = Some(text.into());
            Ok(())
        }
    }
    let probe = Arc::new(Probe(Mutex::new(None)));
    let mut port = Clipboard::default();
    port.bind(probe.clone());
    port.write("hello\nworld").unwrap();
    assert_eq!(port.read().unwrap().as_deref(), Some("hello\nworld"));
}

#[test]
fn composition() {
    let mut page = page();
    page.evaluate("multi.setValue('top\\nbottom'); multi.setSelection(1, 5);")
        .unwrap();
    let id = ime::Id(3);
    assert!(
        page.composition(&ime::Input::Mark {
            id,
            text: "中".into(),
            selection: Selection::caret(1),
        })
        .unwrap()
    );
    page.render().unwrap();
    let marked = page.frame().unwrap();
    Cpu.render(&marked).unwrap();
    page.acknowledge(marked.id()).unwrap();
    assert!(
        page.composition(&ime::Input::Commit {
            id,
            text: "中\r\n文".into(),
        })
        .unwrap()
    );
    assert_eq!(page.evaluate("multi.getValue();").unwrap(), "t中\n文ottom");
    page.render().unwrap();
    let committed = page.frame().unwrap();
    Cpu.render(&committed).unwrap();
}
