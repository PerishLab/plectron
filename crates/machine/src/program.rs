#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Argument(usize),
    Constant(f64),
    Negate(usize),
    Add(usize, usize),
    Subtract(usize, usize),
    Multiply(usize, usize),
    Divide(usize, usize),
}

pub struct Program {
    arity: usize,
    operations: Vec<Operation>,
}

impl Program {
    pub fn new(arity: usize, operations: Vec<Operation>) -> Result<Self, String> {
        if arity > 16 || operations.is_empty() || operations.len() > 256 {
            return Err("numeric program budget exceeded".into());
        }
        for (index, operation) in operations.iter().enumerate() {
            let valid = match *operation {
                Operation::Argument(slot) => slot < arity,
                Operation::Constant(_) => true,
                Operation::Negate(slot) => slot < index,
                Operation::Add(a, b)
                | Operation::Subtract(a, b)
                | Operation::Multiply(a, b)
                | Operation::Divide(a, b) => a < index && b < index,
            };
            if !valid {
                return Err("numeric operand does not dominate its use".into());
            }
        }
        Ok(Self { arity, operations })
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn operations(&self) -> &[Operation] {
        &self.operations
    }
}
