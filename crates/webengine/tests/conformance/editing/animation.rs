use super::super::{File, Source};
use domengine::Extent;
use graphics::shared::{Backend, Cpu};
use webengine::Page;

fn attach(page: &mut Page) {
    page.evaluate(&fixtures::script("document", "const editor = document.getElementById('editor'); const text = document.createTextNode(''); const caret = document.createElement('p-caret'); editor.appendChild(text); editor.appendChild(caret); document.attachEditor(editor, text, caret);")).unwrap();
}

#[test]
fn context() {
    let mut source = Source {
        requested: Vec::new(),
        files: [
            ("/site/index.html".into(), File {
                mime: "text/html".into(),
                body: "<html><head><link rel='stylesheet' href='./style.css'></head><body><p-input id='editor' style='width: 120px; height: 24px;'></p-input></body></html>".into(),
            }),
            ("/site/style.css".into(), File {
                mime: "text/css".into(),
                body: "p-input {overflow: hidden;} p-caret {animation-name: none; opacity: 0;}".into(),
            }),
        ].into_iter().collect(),
    };
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 160,
            height: 60,
        },
    )
    .unwrap();
    page.load(&mut source).unwrap();
    attach(&mut page);
    page.evaluate("document.getElementById('editor').focus();")
        .unwrap();
    page.render().unwrap();
    let empty = page.frame().unwrap();
    let before = Cpu.render(&empty).unwrap();
    let focus = page.focus(&empty).unwrap();
    assert_eq!(focus.text, "");
    page.acknowledge(empty.id()).unwrap();
    page.edit(&webengine::Edit::Insert("abc".into())).unwrap();
    page.render().unwrap();
    let typed = page.frame().unwrap();
    let focus = page.focus(&typed).unwrap();
    assert_eq!(focus.text, "abc");
    assert_eq!(focus.selection.focus, 3);
    assert_ne!(Cpu.render(&typed).unwrap(), before);
    page.acknowledge(typed.id()).unwrap();
    let id = webengine::ime::Id(1);
    assert!(
        page.composition(&webengine::ime::Input::Mark {
            id,
            text: "中".into(),
            selection: webengine::Selection::caret(1),
        })
        .unwrap()
    );
    page.render().unwrap();
    let marked = page.frame().unwrap();
    assert_ne!(Cpu.render(&marked).unwrap(), before);
    page.acknowledge(marked.id()).unwrap();
    assert!(
        page.composition(&webengine::ime::Input::Commit {
            id,
            text: "中".into()
        })
        .unwrap()
    );
    page.render().unwrap();
    let committed = page.frame().unwrap();
    let focus = page.focus(&committed).unwrap();
    assert_eq!(focus.text, "abc中");
    assert_eq!(focus.selection.focus, 4);
}

#[test]
fn caret() {
    let mut source = Source {
        requested: Vec::new(),
        files: [
            ("/site/index.html".into(), File {
                mime: "text/html".into(),
                body: "<html><head><link rel='stylesheet' href='./style.css'></head><body><p-input id='editor'></p-input></body></html>".into(),
            }),
            ("/site/style.css".into(), File {
                mime: "text/css".into(),
                body: "@keyframes blink {from {opacity: 1;} 50% {opacity: 0;} to {opacity: 1;}} p-caret {animation-name: blink; animation-duration: 1200ms; animation-iteration-count: infinite; animation-timing-function: step-end;}".into(),
            }),
        ].into_iter().collect(),
    };
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 120,
            height: 60,
        },
    )
    .unwrap();
    page.load(&mut source).unwrap();
    attach(&mut page);
    assert_eq!(page.evaluate("editor.children().length;").unwrap(), "2");
    assert_eq!(page.evaluate("caret.tagName();").unwrap(), "p-caret");
    assert_eq!(
        page.evaluate("caret.style === editor.children()[1].style;")
            .unwrap(),
        "true"
    );
    assert_eq!(
        page.evaluate("document.getComputedStyle(caret).animationName;")
            .unwrap(),
        "blink"
    );
    page.evaluate("caret.style.setProperty('animation-name', 'none'); caret.style.setProperty('opacity', '0');").unwrap();
    assert_eq!(
        page.evaluate("document.getComputedStyle(caret).opacity;")
            .unwrap(),
        "0"
    );
    page.evaluate("editor.focus(); editor.removeChild(caret);")
        .unwrap();
    assert_eq!(page.evaluate("editor.children().length;").unwrap(), "1");
    page.evaluate("editor.setValue('中');").unwrap();
    assert_eq!(page.evaluate("editor.children().length;").unwrap(), "2");
    assert_eq!(
        page.evaluate("editor.children()[1] === caret;").unwrap(),
        "true"
    );
}

#[test]
fn rules() {
    let mut source = Source {
        requested: Vec::new(),
        files: [
            ("/site/index.html".into(), File {
                mime: "text/html".into(),
                body: "<html><head><link rel='stylesheet' href='./style.css'></head><body><p-input id='editor'></p-input></body></html>".into(),
            }),
            ("/site/style.css".into(), File {
                mime: "text/css".into(),
                body: "@keyframes pulse {from {opacity: 1;} to {opacity: 0;}} p-caret {animation-name: pulse; animation-duration: 1s; animation-iteration-count: infinite; animation-timing-function: step-end;}".into(),
            }),
        ].into_iter().collect(),
    };
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 120,
            height: 60,
        },
    )
    .unwrap();
    page.load(&mut source).unwrap();
    attach(&mut page);
    assert_eq!(
        page.evaluate(
            "const rule = document.keyframes('pulse'); rule === document.keyframes('pulse');"
        )
        .unwrap(),
        "true"
    );
    assert_eq!(page.evaluate("rule.revision();").unwrap(), "0");
    assert_eq!(page.evaluate("rule.opacity(0);").unwrap(), "1");
    page.evaluate("rule.setOpacity(0, 0);").unwrap();
    assert_eq!(page.evaluate("rule.revision();").unwrap(), "1");
    assert_eq!(
        page.evaluate(
            "document.getComputedStyle(document.getElementById('editor').children()[1]).opacity;"
        )
        .unwrap(),
        "0"
    );
    page.render().unwrap();
    assert_eq!(page.evaluate("rule.opacity(0);").unwrap(), "0");
}
