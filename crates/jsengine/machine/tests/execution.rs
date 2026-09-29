#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use machine::Code;
use machine::{Operation as Op, Program};

#[test]
fn validation() {
    assert!(Program::new(0, vec![]).is_err());
    assert!(Program::new(0, vec![Op::Argument(0)]).is_err());
    assert!(Program::new(0, vec![Op::Negate(0)]).is_err());
    assert!(Program::new(17, vec![Op::Constant(1.0)]).is_err());
    assert!(Program::new(0, vec![Op::Constant(1.0); 257]).is_err());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn execution() {
    let program = Program::new(
        2,
        vec![
            Op::Argument(0),
            Op::Argument(1),
            Op::Multiply(0, 1),
            Op::Constant(2.0),
            Op::Add(2, 3),
        ],
    )
    .unwrap();
    let code = Code::compile(&program).unwrap();
    assert_eq!(code.run(&[3.0, 4.0]).unwrap(), 14.0);
    assert!(code.run(&[3.0]).is_err());
    let other = Code::compile(&Program::new(0, vec![Op::Constant(-0.0)]).unwrap()).unwrap();
    assert_eq!(other.run(&[]).unwrap().to_bits(), (-0.0f64).to_bits());
    drop(code);
    assert_eq!(other.run(&[]).unwrap().to_bits(), (-0.0f64).to_bits());
    std::thread::spawn(move || assert_eq!(other.run(&[]).unwrap().to_bits(), (-0.0f64).to_bits()))
        .join()
        .unwrap();
}
