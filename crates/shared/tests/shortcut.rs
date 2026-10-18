use shared::shortcut::{Binding, Chord, Fault, Input, Outcome, Scope, Set, Stroke, dispatch};

fn stroke(repeat: bool) -> Stroke {
    Stroke {
        chord: Chord::parse("Primary+KeyA").unwrap(),
        repeat,
        text: "a".into(),
    }
}
fn set(priority: i32, command: u8) -> Set<u8> {
    Set {
        priority,
        bindings: vec![Binding {
            chord: stroke(false).chord,
            command,
            repeat: false,
            consume: true,
        }],
    }
}
#[test]
fn lifecycle() {
    let mut scope = Scope::default();
    let mut foreign = Scope::<u8>::default();
    let handle = scope.register(set(0, 1)).unwrap();
    assert_eq!(foreign.remove(&handle), Err(Fault::Handle));
    let mut invalid = set(99, 2);
    invalid.bindings[0].chord.code = "unknown".into();
    assert_eq!(scope.replace(&handle, invalid), Err(Fault::Input));
    assert_eq!(scope.candidates(&stroke(false))[0].command, 1);
    scope.replace(&handle, set(5, 3)).unwrap();
    assert_eq!(scope.candidates(&stroke(false))[0].command, 3);
    scope.remove(&handle).unwrap();
    assert!(scope.candidates(&stroke(false)).is_empty());
    let replacement = scope.register(set(0, 4)).unwrap();
    assert_eq!(scope.remove(&handle), Err(Fault::Handle));
    assert_eq!(scope.replace(&handle, set(0, 5)), Err(Fault::Handle));
    scope.remove(&replacement).unwrap();
    let value = std::sync::Arc::new(());
    let weak = std::sync::Arc::downgrade(&value);
    let mut owned = Scope::default();
    let handle = owned
        .register(Set {
            priority: 0,
            bindings: vec![Binding {
                chord: stroke(false).chord,
                command: value,
                repeat: true,
                consume: true,
            }],
        })
        .unwrap();
    drop(owned);
    assert!(weak.upgrade().is_none());
    assert!(Scope::<()>::default().remove(&handle).is_err());
}
#[test]
fn resolution() {
    let mut scope = Scope::default();
    scope.register(set(0, 1)).unwrap();
    scope.register(set(20, 2)).unwrap();
    scope.register(set(20, 3)).unwrap();
    let mut seen = Vec::new();
    let outcome = dispatch(scope.candidates(&stroke(false)), |command| {
        seen.push(command);
        Ok::<_, ()>(command == 1)
    })
    .unwrap();
    assert_eq!(seen, [2, 3, 1]);
    assert_eq!(
        outcome,
        Outcome {
            handled: true,
            consumed: true
        }
    );
    assert!(scope.candidates(&stroke(true)).is_empty());
    let mut repeated = set(50, 4);
    repeated.bindings[0].repeat = true;
    repeated.bindings[0].consume = false;
    scope.register(repeated).unwrap();
    assert_eq!(
        dispatch(scope.candidates(&stroke(true)), |_| Ok::<_, ()>(true)).unwrap(),
        Outcome {
            handled: true,
            consumed: false
        }
    );
    let mut seen = Vec::new();
    assert_eq!(
        dispatch(scope.candidates(&stroke(false)), |command| {
            seen.push(command);
            Err::<bool, _>("fault")
        }),
        Err("fault")
    );
    assert_eq!(seen, [4]);
}
#[test]
fn budgets() {
    let mut scope = Scope::default();
    for _ in 0..32 {
        scope.register(set(0, 1)).unwrap();
    }
    assert!(matches!(scope.register(set(0, 2)), Err(Fault::Budget)));
    assert!(Chord::parse("Shift+Shift+KeyA").is_err());
    assert!(Chord::parse("Control+Unknown").is_err());
    assert_ne!(
        Chord::parse("Primary+KeyA").unwrap(),
        Chord::parse("Shift+Primary+KeyA").unwrap()
    );
}
#[test]
fn arrows() {
    for code in ["ArrowUp", "ArrowDown", "Shift+ArrowUp", "Primary+ArrowDown"] {
        let chord = Chord::parse(code).unwrap();
        let input = Input::Key(Stroke {
            chord,
            repeat: false,
            text: String::new(),
        });
        assert_eq!(input.validate(), Ok(()));
    }
    assert!(Chord::parse("ArrowForward").is_err());
    let unknown = Input::Key(Stroke {
        chord: Chord {
            code: "ArrowForward".into(),
            modifiers: Default::default(),
        },
        repeat: false,
        text: String::new(),
    });
    assert_eq!(unknown.validate(), Err(Fault::Input));
}
