use super::lowering::Lowering;
use super::report::Function;
use crate::syntax;
use machine::Code;
use std::time::Instant;

pub(super) struct Compiled {
    pub code: Code,
    pub lowering: Lowering,
}

#[derive(Default)]
pub(super) struct Entry {
    pub report: Function,
    pub compiled: Option<Compiled>,
}

impl Entry {
    pub fn compile(&mut self, code: &syntax::Function) {
        let start = Instant::now();
        let result = Lowering::new(code).and_then(|lowering| {
            Code::compile(&lowering.program).map(|code| Compiled { code, lowering })
        });
        self.report.compilation = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        match result {
            Ok(compiled) => {
                self.report.bytes = compiled.code.bytes();
                self.compiled = Some(compiled);
            }
            Err(reason) => self.report.rejection = Some(reason),
        }
    }
}
