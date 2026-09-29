use crate::syntax::{Expression, Function, Statement};
use machine::{Operation, Program};

pub(super) struct Lowering {
    pub program: Program,
    pub fuel: usize,
    pub depth: usize,
}

struct Builder<'a> {
    code: &'a Function,
    operations: Vec<Operation>,
    nodes: usize,
    depth: usize,
}

impl Lowering {
    pub fn new(code: &Function) -> Result<Self, String> {
        if code.asynchronous {
            return Err("async activation requires interpretation".into());
        }
        if code.params.rest.is_some()
            || code
                .params
                .items
                .iter()
                .any(|item| item.initializer.is_some())
        {
            return Err("parameter initialization requires interpretation".into());
        }
        let [Statement::Return(Some(expression))] = code.body.as_ref() else {
            return Err("body is not a single numeric return".into());
        };
        let mut builder = Builder {
            code,
            operations: Vec::new(),
            nodes: 0,
            depth: 0,
        };
        builder.expression(expression, 1)?;
        Ok(Self {
            program: Program::new(code.params.items.len(), builder.operations)?,
            fuel: code.params.items.len() + 1 + builder.nodes,
            depth: builder.depth,
        })
    }
}

impl Builder<'_> {
    fn expression(&mut self, expression: &Expression, depth: usize) -> Result<usize, String> {
        self.nodes += 1;
        self.depth = self.depth.max(depth);
        if self.nodes > 256 || depth > 128 {
            return Err("numeric lowering budget exceeded".into());
        }
        let operation = match expression {
            Expression::Number(value) => Operation::Constant(*value),
            Expression::Name(name) => {
                let slot = self
                    .code
                    .params
                    .items
                    .iter()
                    .position(|item| &item.name == name)
                    .ok_or("captured bindings require interpretation")?;
                Operation::Argument(slot)
            }
            Expression::Unary(operator, value) if operator == "+" => {
                return self.expression(value, depth + 1);
            }
            Expression::Unary(operator, value) if operator == "-" => {
                Operation::Negate(self.expression(value, depth + 1)?)
            }
            Expression::Binary(operator, a, b) => {
                let a = self.expression(a, depth + 1)?;
                let b = self.expression(b, depth + 1)?;
                arithmetic(operator, a, b)?
            }
            _ => return Err("expression requires interpretation".into()),
        };
        let slot = self.operations.len();
        self.operations.push(operation);
        Ok(slot)
    }
}

fn arithmetic(operator: &str, a: usize, b: usize) -> Result<Operation, String> {
    Ok(match operator {
        "+" => Operation::Add(a, b),
        "-" => Operation::Subtract(a, b),
        "*" => Operation::Multiply(a, b),
        "/" => Operation::Divide(a, b),
        _ => return Err("operator requires interpretation".into()),
    })
}
