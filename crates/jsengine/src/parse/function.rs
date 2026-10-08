use super::{Context, Parser, Rejection};
use crate::syntax::{Function, Parameter, Parameters, Statement};

impl Parser {
    pub(super) fn parameters(&mut self) -> Result<Parameters, Rejection> {
        self.expect("(")?;
        let mut params = Parameters::default();
        while !self.eat(")") {
            let rest = self.eat("...");
            let name = self.name()?;
            if params.names().any(|previous| previous == &name) {
                return Err(self.fault("duplicate parameter"));
            }
            if rest {
                self.expect(")")?;
                params.rest = Some(name);
                return Ok(params);
            }
            let initializer = if self.eat("=") {
                Some(self.initializing()?)
            } else {
                None
            };
            params.items.push(Parameter { name, initializer });
            if self.eat(")") {
                break;
            }
            self.expect(",")?;
        }
        Ok(params)
    }

    pub(super) fn function(&mut self, context: Context) -> Result<Function, Rejection> {
        let awaiting = self.asynchronous;
        self.asynchronous = false;
        let previous = self.context;
        self.context = context;
        let result = self.definition();
        self.context = previous;
        self.asynchronous = awaiting;
        result
    }

    fn definition(&mut self) -> Result<Function, Rejection> {
        let params = self.parameters()?;
        let body = self.activation(false)?;
        finish(params, body)
    }

    pub(super) fn activation(&mut self, concise: bool) -> Result<Vec<Statement>, Rejection> {
        let loops = self.loops;
        self.loops = 0;
        self.functions += 1;
        let body = if concise && !self.at("{") {
            let start = self.tokens.peek().span.start;
            self.expression(0).map(|value| {
                let end = self.tokens.previous().map_or(start, |token| token.span.end);
                vec![self.located(Statement::Return(Some(value)), start..end)]
            })
        } else {
            self.expect("{").and_then(|()| self.body())
        };
        self.functions -= 1;
        self.loops = loops;
        body
    }
}

pub(super) fn finish(params: Parameters, body: Vec<Statement>) -> Result<Function, Rejection> {
    let names = params.names().cloned().collect::<Vec<_>>();
    super::validate(&body, &names)?;
    Ok(Function {
        params: params.into(),
        body: body.into(),
        asynchronous: false,
    })
}
