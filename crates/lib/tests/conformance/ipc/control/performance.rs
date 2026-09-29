use super::{Debugger, setup};
use crate::web::call;
use plectron::ipc::Command;

#[test]
fn publication() {
    let _guard = super::serial();
    let (mut session, _) = setup("const calculate = (x, y) => x * y + 2;");
    let calls = "calculate(3, 4);".repeat(40);
    assert_eq!(session.evaluate(&calls).unwrap(), "14");
    let before = session.inspect().performance;
    let compiled = before
        .functions
        .iter()
        .find(|function| function.calls == 40)
        .unwrap();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    assert_eq!(compiled.native, 9);
    assert!(session.evaluate("calculate({}, 2);").is_err());
    let snapshot = session.inspect();
    let after = snapshot.performance;
    let failed = after
        .functions
        .iter()
        .find(|function| function.calls == 41)
        .unwrap();
    assert_eq!(failed.native, compiled.native);
    assert!(snapshot.activity.calls >= 41);
    call(&mut session, Command::Shutdown {});
}
