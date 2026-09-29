use jsengine::{Addons, Engine, Failure, Fault, Value};

#[derive(Default)]
struct Harness {
    assertions: usize,
    error: Option<Value>,
}

pub fn run(source: &str) -> crate::Result<usize> {
    Engine::check(source).map_err(|error| error.to_string())?;
    let mut engine = Engine::default();
    let mut harness = Harness::default();
    harness
        .install(&mut engine)
        .map_err(|error| error.to_string())?;
    engine
        .execute(source, &mut harness)
        .map_err(|failure| harness.diagnostic(&engine, &failure))?;
    Ok(harness.assertions)
}

impl Harness {
    fn diagnostic(&self, engine: &Engine, failure: &Failure) -> String {
        if let Failure::Throw(value) = failure
            && engine.get(value, "constructor").ok() == self.error
            && let Ok(Value::Text(message)) = engine.get(value, "message")
        {
            return format!("uncaught Test262Error: {message}");
        }
        engine.diagnostic(failure).to_string()
    }

    fn install(&mut self, engine: &mut Engine) -> Result<(), Failure> {
        let source = source()?;
        self.error = Some(engine.execute(source, self)?);
        let assert = engine.native(0);
        for (index, name) in ["sameValue", "notSameValue", "throws"]
            .into_iter()
            .enumerate()
        {
            let method = engine.native(index + 1);
            engine.set(&assert, name, method)?;
        }
        engine.register("assert", assert)?;
        Ok(())
    }

    fn throws(
        &mut self,
        engine: &mut Engine,
        expected: &Value,
        callback: &Value,
    ) -> Result<bool, Failure> {
        if engine.callable(callback).is_err() {
            return Ok(false);
        }
        match engine.call(callback.clone(), Vec::new(), self) {
            Err(Failure::Throw(value @ Value::Object(_))) => {
                if engine.callable(&value).is_ok() {
                    return Ok(false);
                }
                Ok(
                    engine.read(&jsengine::Access::direct(&value, "constructor"), self)?
                        == *expected,
                )
            }
            Err(Failure::Throw(_)) | Ok(_) => Ok(false),
            Err(failure) => Err(failure),
        }
    }
}

impl Addons for Harness {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        self.assertions += 1;
        let actual = invocation.arguments.first().unwrap_or(&Value::Undefined);
        let expected = invocation.arguments.get(1).unwrap_or(&Value::Undefined);
        let passed = match binding {
            0 => *actual == Value::Boolean(true),
            1 => same(actual, expected),
            2 => !same(actual, expected),
            3 => self.throws(engine, actual, expected)?,
            _ => return Err(Fault("unknown assertion binding".into()).into()),
        };
        if !passed {
            let constructor = self
                .error
                .clone()
                .ok_or_else(|| Fault("harness is not installed".into()))?;
            let message = Value::Text(format!(
                "assertion {} failed (binding {binding})",
                self.assertions
            ));
            let error = engine.construct(constructor, vec![message], self)?;
            return Err(Failure::Throw(error));
        }
        Ok(Value::Undefined)
    }
}

fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

fn source() -> Result<&'static str, Fault> {
    shared::Resource::new(include_bytes!("../resources/harness.js"))
        .text()
        .map_err(|e| Fault(e.to_string()))
}
