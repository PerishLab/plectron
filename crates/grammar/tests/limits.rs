use grammar::{Budget, Cursor, Grammar, Kind};

#[test]
fn nullable() {
    let grammar = Grammar::compile("a = 'x'?;").unwrap();
    let mut cursor = Cursor::new("x").unwrap();
    assert_eq!(
        grammar.lex(&["a"], &mut cursor).unwrap_err().kind,
        Kind::Grammar
    );
    assert_eq!(cursor.offset(), 0);
    assert_eq!(grammar.require("a", &mut cursor).unwrap().text, "x");
}

#[test]
fn structure() {
    let rules = (0..257)
        .map(|index| format!("a{index} = 'x';"))
        .collect::<String>();
    for source in [
        " ".repeat(65_537),
        format!("a = {}'x'{};", "(".repeat(65), ")".repeat(65)),
        format!("a = {};", "|".repeat(4096)),
        rules,
    ] {
        assert_eq!(Grammar::compile(&source).err().unwrap().kind, Kind::Budget);
    }
}

#[test]
fn locations() {
    let grammar = Grammar::compile("look = &'雪'; next = '雪\n'; fail = 'x' 'y';").unwrap();
    let mut cursor = Cursor::new("雪\nxz").unwrap();
    grammar.require("look", &mut cursor).unwrap();
    assert_eq!(cursor.offset(), 0);
    grammar.require("next", &mut cursor).unwrap();
    assert_eq!(
        (
            cursor.offset(),
            cursor.location().line,
            cursor.location().column
        ),
        (4, 1, 0)
    );
    assert_eq!(grammar.require("fail", &mut cursor).unwrap_err().offset, 4);
    assert_eq!(cursor.offset(), 4);
    assert_eq!(cursor.take().unwrap(), Some('x'));
    assert_eq!(
        cursor.source().location(cursor.offset()),
        Some(cursor.location())
    );
    assert!(cursor.advance(usize::MAX).is_err());
    assert_eq!(cursor.offset(), 5);
}

#[test]
fn candidates() {
    let grammar = Grammar::compile("a = 'a'; b = 'aaaa';").unwrap();
    let mut cursor = Cursor::bounded(
        "aaaa",
        Budget {
            work: 5,
            ..Budget::default()
        },
    )
    .unwrap();
    assert_eq!(
        grammar.lex(&["a", "b"], &mut cursor).unwrap_err().kind,
        Kind::Budget
    );
    assert_eq!(cursor.offset(), 0);
    assert_eq!(cursor.remaining(), 0);
    let mut cursor = Cursor::new("a").unwrap();
    let before = cursor.remaining();
    assert_eq!(
        grammar
            .lex(&["a", "missing"], &mut cursor)
            .unwrap_err()
            .kind,
        Kind::Grammar
    );
    assert_eq!(cursor.remaining(), before);
}
