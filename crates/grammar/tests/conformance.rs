use grammar::{Budget, Cursor, Grammar, Kind, Source, Span, Stream};

#[test]
fn unicode() {
    let grammar = Grammar::compile("word = [^:]+; colon = ':';").unwrap();
    let mut cursor = Cursor::new("雪é:🦀\n甲").unwrap();
    let word = grammar.require("word", &mut cursor).unwrap();
    assert_eq!(word.span, Span { start: 0, end: 5 });
    assert_eq!(word.text, "雪é");
    assert_eq!(cursor.source().slice(word.span), Some("雪é"));
    grammar.require("colon", &mut cursor).unwrap();
    assert_eq!(cursor.take().unwrap(), Some('🦀'));
    cursor.take().unwrap();
    let location = cursor.source().location(cursor.offset()).unwrap();
    assert_eq!(
        (location.character, location.line, location.column),
        (5, 1, 0)
    );
    assert_eq!(Source::new("雪").location(1), None);
    assert!(cursor.advance(1).is_err());
}

#[test]
fn alternatives() {
    let grammar =
        Grammar::compile("short = '='; long = '==='; same = '==='; choice = '=' | '===';").unwrap();
    let mut cursor = Cursor::new("===").unwrap();
    let (name, token) = grammar
        .lex(&["same", "long", "short"], &mut cursor)
        .unwrap()
        .unwrap();
    assert_eq!((name, token.text), ("long", "==="));
    let mut cursor = Cursor::new("===").unwrap();
    assert_eq!(grammar.require("choice", &mut cursor).unwrap().text, "=");
}

#[test]
fn rollback() {
    let grammar = Grammar::compile("failed = '雪' 'x'; word = '雪' 'y';").unwrap();
    let mut cursor = Cursor::new("雪y").unwrap();
    let before = cursor.remaining();
    assert!(grammar.matches("failed", &mut cursor).unwrap().is_none());
    assert_eq!(cursor.offset(), 0);
    assert_eq!(cursor.location().character, 0);
    assert!(cursor.remaining() < before);
    assert_eq!(grammar.require("word", &mut cursor).unwrap().text, "雪y");
}

#[test]
fn handoff() {
    let grammar = Grammar::compile("open = '('; digits = [0-9]+; close = ')';").unwrap();
    let mut cursor = Cursor::new("(12+3)").unwrap();
    grammar.require("open", &mut cursor).unwrap();
    let left: usize = grammar
        .require("digits", &mut cursor)
        .unwrap()
        .text
        .parse()
        .unwrap();
    assert_eq!(cursor.take().unwrap(), Some('+'));
    let right: usize = grammar
        .require("digits", &mut cursor)
        .unwrap()
        .text
        .parse()
        .unwrap();
    grammar.require("close", &mut cursor).unwrap();
    assert_eq!(left + right, 15);
    assert!(cursor.rest().is_empty());
}

#[test]
fn lookahead() {
    let grammar = Grammar::compile("body = (!'*/' .)*; end = &'*/' '*/'; space = [\\s]*;").unwrap();
    let mut cursor = Cursor::new("\u{2003}a雪*/").unwrap();
    grammar.require("space", &mut cursor).unwrap();
    assert_eq!(grammar.require("body", &mut cursor).unwrap().text, "a雪");
    assert_eq!(grammar.require("end", &mut cursor).unwrap().text, "*/");
}

#[test]
fn invalid() {
    for source in [
        "",
        "a = missing;",
        "a = 'x'; a = 'y';",
        "a = a 'x';",
        "a = b 'x'; b = a?;",
        "a = ''*;",
        "a = b+; b = '';",
        "a = (!'x')*;",
        "a = [z-a];",
        "a = [];",
        "a = 'x'",
        "a = ('x';",
        "a = [abc;",
        "a = '\\q';",
        "a = 'x'; trailing",
    ] {
        let error = Grammar::compile(source).err().unwrap();
        assert_eq!(error.kind, Kind::Grammar, "{source}: {error}");
    }
}

#[test]
fn recursive() {
    let grammar = Grammar::compile("list = '(' list? ')' | 'x';").unwrap();
    let mut cursor = Cursor::new("((x))").unwrap();
    assert_eq!(grammar.require("list", &mut cursor).unwrap().text, "((x))");
    let source = format!("{}x{}", "(".repeat(100), ")".repeat(100));
    let mut cursor = Cursor::new(&source).unwrap();
    assert_eq!(
        grammar.require("list", &mut cursor).unwrap_err().kind,
        Kind::Budget
    );
    assert_eq!(cursor.offset(), 0);
}

#[test]
fn exhaustion() {
    let grammar = Grammar::compile("a = ('a' 'b' | 'a' 'c')*;").unwrap();
    let mut cursor = Cursor::bounded(
        "acac",
        Budget {
            work: 12,
            ..Budget::default()
        },
    )
    .unwrap();
    assert_eq!(
        grammar.require("a", &mut cursor).unwrap_err().kind,
        Kind::Budget
    );
    assert_eq!(cursor.offset(), 0);
    assert_eq!(cursor.remaining(), 0);
    assert_eq!(
        grammar.require("a", &mut cursor).unwrap_err().kind,
        Kind::Budget
    );
    assert!(
        Cursor::bounded(
            "雪",
            Budget {
                bytes: 2,
                ..Budget::default()
            }
        )
        .is_err()
    );
}

#[test]
fn failure() {
    let grammar = Grammar::compile("a = 'a'; empty = '';").unwrap();
    let mut cursor = Cursor::new("?").unwrap();
    assert!(grammar.lex(&["a"], &mut cursor).unwrap().is_none());
    assert_eq!(
        grammar.require("a", &mut cursor).unwrap_err().kind,
        Kind::Syntax
    );
    assert_eq!(
        grammar.matches("absent", &mut cursor).unwrap_err().kind,
        Kind::Grammar
    );
    assert_eq!(
        grammar.lex(&["empty"], &mut cursor).unwrap_err().kind,
        Kind::Grammar
    );
    assert_eq!(cursor.offset(), 0);
}

#[test]
fn stream() {
    let mut stream = Stream::new(vec![1, 2], 0);
    assert_eq!(*stream.look(1), 2);
    assert_eq!(*stream.look(usize::MAX), 0);
    assert_eq!(stream.previous(), None);
    assert!(!stream.eat(|value| *value == 2));
    assert_eq!(*stream.take(), 1);
    assert_eq!(stream.previous(), Some(&1));
    assert_eq!(*stream.look(usize::MAX), 0);
    assert_eq!(*stream.look(0), 2);
    assert!(stream.eat(|value| *value == 2));
    assert_eq!(*stream.take(), 0);
    assert_eq!(*stream.take(), 0);
    assert!(!stream.eat(|value| *value == 0));
}
