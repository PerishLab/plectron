use super::{Fault, Value};

pub(super) fn unary(operator: &str, value: Value) -> Result<Value, Fault> {
    match operator {
        "!" => Ok(Value::Boolean(!value.truth())),
        "-" => Ok(Value::Number(-value.number()?)),
        "+" => Ok(Value::Number(value.number()?)),
        _ => Err(Fault(format!("unsupported unary operator: {operator}"))),
    }
}

pub(super) fn binary(operator: &str, left: Value, right: Value) -> Result<Value, Fault> {
    match operator {
        "&&" | "||" => Ok(right),
        "===" => Ok(Value::Boolean(left == right)),
        "!==" => Ok(Value::Boolean(left != right)),
        "+" => add(left, right),
        "-" => Ok(Value::Number(left.number()? - right.number()?)),
        "*" => Ok(Value::Number(left.number()? * right.number()?)),
        "/" => Ok(Value::Number(left.number()? / right.number()?)),
        "%" => Ok(Value::Number(left.number()? % right.number()?)),
        "<" | "<=" | ">" | ">=" => Ok(Value::Boolean(compare(operator, left, right)?)),
        _ => Err(Fault(format!("unsupported binary operator: {operator}"))),
    }
}

fn add(left: Value, right: Value) -> Result<Value, Fault> {
    if matches!(left, Value::Object(_)) {
        return Err(Fault("object coercion is unsupported".into()));
    }
    if matches!(right, Value::Object(_)) {
        return Err(Fault("object coercion is unsupported".into()));
    }
    if matches!(left, Value::Text(_)) || matches!(right, Value::Text(_)) {
        let left = left.text();
        let right = right.text();
        if left.len().saturating_add(right.len()) > 1_048_576 {
            return Err(Fault("string budget exhausted".into()));
        }
        return Ok(Value::Text(left + &right));
    }
    Ok(Value::Number(left.number()? + right.number()?))
}

fn compare(operator: &str, left: Value, right: Value) -> Result<bool, Fault> {
    let order = match (&left, &right) {
        (Value::Text(left), Value::Text(right)) => left.chars().partial_cmp(right.chars()),
        _ => left.number()?.partial_cmp(&right.number()?),
    };
    Ok(match operator {
        "<" => order == Some(std::cmp::Ordering::Less),
        "<=" => matches!(
            order,
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ),
        ">" => order == Some(std::cmp::Ordering::Greater),
        ">=" => matches!(
            order,
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ),
        _ => false,
    })
}
