use super::{Debugger, serial};
use crate::{device::Device, web::call};
use plectron::{
    ipc::{Command, Session},
    platform::Platform,
    rendering::Solid,
};

#[test]
fn callbacks() {
    let _guard = serial();
    let entry = project::Module::new("entry.ts").unwrap();
    let text = "const saved = function (): never { throw 'retained'; };";
    let output = project::Linker::new(
        project::Memory::new([(entry.clone(), text.into())]),
        project::Resolver::new([]).unwrap(),
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .link(entry, project::Target::Script)
    .unwrap();
    let script = plectron::runtime::source::script(output);
    let device = Device::default();
    let platform = Platform::threaded(device, Solid);
    let mut session = Session::spawn(platform, script, (42, false), None).unwrap();
    session
        .until(|session| session.inspect().jobs[0].phase() == plectron::control::Phase::Complete);
    assert!(session.inspect().jobs[0].result().unwrap().is_ok());
    session.evaluate("const unrelated = 1;").unwrap();
    let failure = session.evaluate("saved();").unwrap_err();
    let start = text.find("throw").unwrap();
    let end = start + "throw 'retained';".len();
    assert_eq!(
        failure,
        format!("entry.ts:{start}..{end}: uncaught exception: retained")
    );
    call(&mut session, Command::Shutdown {});
}
