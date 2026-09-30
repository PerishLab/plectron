use crate::{Addons, Engine, Error, Failure, Rejection, Source, Value, eval, parse};

impl Engine {
    pub fn check(source: &str) -> Result<(), Rejection> {
        parse::program(source, None).map(|_| ())
    }

    pub fn execute(&mut self, text: &str, addons: &mut impl Addons) -> Result<Value, Failure> {
        observe::call("js.execute", || self.script((text, None), addons))
    }

    pub fn source(&mut self, source: &Source, addons: &mut impl Addons) -> Result<Value, Failure> {
        observe::call("js.execute", || {
            self.script((source.text(), Some(source)), addons)
        })
    }

    fn script(
        &mut self,
        input: (&str, Option<&Source>),
        addons: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let previous = self.site.take();
        let result = self.program(input, addons);
        self.site = previous;
        result
    }

    fn program(
        &mut self,
        input: (&str, Option<&Source>),
        addons: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let program = observe::call("js.parse", || parse::program(input.0, input.1)).map_err(
            |rejection| {
                let site = input
                    .1
                    .zip(rejection.span())
                    .map(|(source, span)| source.site(span));
                match rejection {
                    Rejection::Syntax(syntax) => self.raise(Error::Syntax, syntax.message).at(site),
                    Rejection::Fault(fault) => Failure::Fault(fault),
                }
            },
        )?;
        observe::call("js.admission", || self.admit(&program))?;
        self.enter();
        let result = observe::call("js.evaluate", || self.block(&program, 0, addons));
        self.turns -= 1;
        match result? {
            eval::Flow::Next(value) => Ok(value.unwrap_or(Value::Undefined)),
            eval::Flow::Throw(failure) => Err(failure),
            _ => Err(crate::Fault("control escaped script".into()).into()),
        }
    }
}
