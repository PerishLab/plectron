use crate::{Operation, Program};

pub(super) fn emit(program: &Program) -> Vec<u32> {
    let mut words = Vec::new();
    for (index, operation) in program.operations().iter().enumerate() {
        match *operation {
            Operation::Argument(slot) => words.push(load(0, 0, slot)),
            Operation::Constant(value) => constant(&mut words, value),
            Operation::Negate(slot) => {
                words.push(load(0, 1, slot));
                words.push(0x1e614000);
            }
            operation => binary(&mut words, operation),
        }
        words.push(0xfd000020 | ((index as u32) << 10));
    }
    words.push(0xd65f03c0);
    words
}

fn load(register: u32, base: u32, slot: usize) -> u32 {
    0xfd400000 | ((slot as u32) << 10) | (base << 5) | register
}

fn constant(words: &mut Vec<u32>, value: f64) {
    let bits = value.to_bits();
    words.push(0xd2800009 | (((bits & 0xffff) as u32) << 5));
    for shift in 1..4 {
        let immediate = ((bits >> (shift * 16)) & 0xffff) as u32;
        words.push(0xf2800009 | (shift << 21) | (immediate << 5));
    }
    words.push(0x9e670120);
}

fn binary(words: &mut Vec<u32>, operation: Operation) {
    let (a, b, instruction) = match operation {
        Operation::Add(a, b) => (a, b, 0x1e612800),
        Operation::Subtract(a, b) => (a, b, 0x1e613800),
        Operation::Multiply(a, b) => (a, b, 0x1e610800),
        Operation::Divide(a, b) => (a, b, 0x1e611800),
        _ => unreachable!(),
    };
    words.push(load(0, 1, a));
    words.push(load(1, 1, b));
    words.push(instruction);
}
