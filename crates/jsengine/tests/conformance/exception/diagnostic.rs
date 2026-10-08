use super::super::native::Bare;
use jsengine::{Engine, Rejection, Value};

#[test]
fn token() {
    let source = "const text = '𐐀';\nvar next = 1;";
    let error = Engine::check(source).unwrap_err();
    let start = source.find("var").unwrap();
    assert_eq!(error.span(), Some(start..start + 3));
    let Rejection::Syntax(syntax) = error else {
        panic!("expected syntax rejection")
    };
    assert!(syntax.message.contains("unsupported keyword: var"));
    assert_eq!(&source[syntax.span.unwrap()], "var");
}

#[test]
fn identifier() {
    let source = "const 12 = 0;";
    let error = Engine::check(source).unwrap_err();
    assert_eq!(error.span(), Some(6..8));
}

#[test]
fn end() {
    for source in ["const value = ", "const ", "const value = object."] {
        let error = Engine::check(source).unwrap_err();
        assert_eq!(error.span(), Some(source.len()..source.len()));
    }
}

#[test]
fn scanner() {
    let source = "const text = '𐐀';\n#";
    let error = Engine::check(source).unwrap_err();
    assert_eq!(error.span(), Some(source.len() - 1..source.len() - 1));
}

#[test]
fn assignment() {
    let source = "(1 + 2) = 3;";
    let error = Engine::check(source).unwrap_err();
    assert_eq!(error.span(), Some(0..11));
}

#[test]
fn budget() {
    let source = " ".repeat(65_537);
    let error = Engine::check(&source).unwrap_err();
    assert!(matches!(error, Rejection::Fault(_)));
    assert_eq!(error.span(), None);
}

#[test]
fn effects() {
    let mut engine = Engine::default();
    engine.execute("let value = 1;", &mut Bare).unwrap();
    let failure = engine
        .execute("value = 2; var rejected = 0;", &mut Bare)
        .unwrap_err();
    assert!(engine.diagnostic(&failure).0.contains("SyntaxError:"));
    assert_eq!(
        engine.execute("value;", &mut Bare).unwrap(),
        Value::Number(1.0)
    );
}

#[test]
fn absent() {
    let error = Engine::check("let value = 1; let value = 2;").unwrap_err();
    assert!(matches!(error, Rejection::Syntax(_)));
    assert_eq!(error.span(), None);
}
