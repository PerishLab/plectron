use super::{Sample, elapsed};
use crate::Result;
use jsengine::jit::Mode;
use jsengine::{Addons, Engine, Failure, Fault, Invocation, Value};
use std::hint::black_box;
use std::time::Instant;

struct Bare;
impl Addons for Bare {
    fn call(
        &mut self,
        _: &mut Engine,
        _: usize,
        _: Invocation,
    ) -> std::result::Result<Value, Failure> {
        Err(Fault("benchmark has no addons".into()).into())
    }
}

pub(super) fn sample(mode: Mode, repetition: usize) -> Result<Sample> {
    let start = Instant::now();
    let mut engine = Engine::default();
    engine.tier(mode);
    let function = engine
        .execute("(x, y) => x * y + 2;", &mut Bare)
        .map_err(|error| error.to_string())?;
    let startup = elapsed(start);
    let start = Instant::now();
    invoke(&mut engine, &function)?;
    let cold = elapsed(start);
    for _ in 0..32 {
        invoke(&mut engine, &function)?;
    }
    let start = Instant::now();
    for _ in 0..2000 {
        invoke(&mut engine, &function)?;
    }
    Ok(Sample {
        scenario: "numeric-call".into(),
        repetition,
        startup,
        cold,
        iterations: 2000,
        elapsed: elapsed(start),
        report: engine.performance(),
    })
}

fn invoke(engine: &mut Engine, function: &Value) -> Result<()> {
    let arguments = vec![Value::Number(black_box(3.0)), Value::Number(black_box(4.0))];
    let value = engine
        .call(function.clone(), arguments, &mut Bare)
        .map_err(|error| error.to_string())?;
    if black_box(value) != Value::Number(14.0) {
        return Err("numeric benchmark result mismatch".into());
    }
    Ok(())
}
