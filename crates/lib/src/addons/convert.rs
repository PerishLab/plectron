use jsengine::{Fault, Value};

pub(super) fn text(value: &Value) -> Result<String, Fault> {
    match value {
        Value::Text(text) if !text.contains('\0') => Ok(text.clone()),
        _ => Err(Fault("expected string without NUL".into())),
    }
}

pub(super) fn integer(value: &Value, maximum: usize) -> Result<usize, Fault> {
    match value {
        Value::Number(number) if number.is_finite() && number.fract() == 0.0 => {
            if *number >= 0.0 && *number <= maximum as f64 {
                return Ok(*number as usize);
            }
            Err(Fault(format!("integer must be between 0 and {maximum}")))
        }
        _ => Err(Fault("expected finite integer".into())),
    }
}
