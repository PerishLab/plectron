use project::{
    Diagnostic, Import, Linker, Module, Provider, Request, Resolve, Source, Span, Target,
};
use pt::Fault;

pub struct Linked {
    pub javascript: String,
    pub entry: String,
}

pub fn link(
    entry: &str,
    target: Target,
    read: impl Fn(&str) -> Result<String, Fault>,
    resolve: impl Fn(&str, &str) -> Result<String, Fault>,
) -> Result<Linked, Fault> {
    let entry = Module::new(entry).map_err(fault)?;
    let output = Linker::new(Reader(read), Paths(resolve), pt::Templates)
        .link(entry, target)
        .map_err(diagnostic)?;
    let entry = output.graph.binding(&output.graph.entry);
    Ok(Linked {
        javascript: output.javascript,
        entry,
    })
}

struct Reader<R>(R);

impl<R: Fn(&str) -> Result<String, Fault>> Provider for Reader<R> {
    fn read(&self, request: Request) -> Result<Source, Diagnostic> {
        let kind = request.module.kind().map_err(|failure| {
            Diagnostic::new(request.module.clone(), Span::default(), failure.0)
        })?;
        let text = (self.0)(request.module.path()).map_err(|failure| {
            Diagnostic::new(request.module.clone(), Span::default(), failure.0)
        })?;
        Ok(Source {
            module: request.module,
            kind,
            text,
        })
    }
}

struct Paths<R>(R);

impl<R: Fn(&str, &str) -> Result<String, Fault>> Resolve for Paths<R> {
    fn resolve(&self, request: Import) -> Result<Module, Diagnostic> {
        let path = (self.0)(request.from.path(), &request.specifier)
            .map_err(|failure| Diagnostic::new(request.from.clone(), request.span, failure.0))?;
        Module::new(path).map_err(|failure| Diagnostic::new(request.from, request.span, failure.0))
    }
}

fn fault(failure: project::Fault) -> Fault {
    Fault(failure.0)
}

fn diagnostic(failure: Diagnostic) -> Fault {
    Fault(failure.to_string())
}
