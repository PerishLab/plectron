use crate::{Artifact, Fault, Templates};
use project::{
    Diagnostic, Import, Linker, Module, Provider, Request, Resolve, Source, Span, Target,
};

pub fn bundle(
    entry: &str,
    read: impl Fn(&str) -> Result<String, Fault>,
    resolve: impl Fn(&str, &str) -> Result<String, Fault>,
) -> Result<Artifact, Fault> {
    link(entry, read, resolve, Target::Module)
}

pub fn script(
    entry: &str,
    read: impl Fn(&str) -> Result<String, Fault>,
    resolve: impl Fn(&str, &str) -> Result<String, Fault>,
) -> Result<Artifact, Fault> {
    link(entry, read, resolve, Target::Script)
}

fn link(
    entry: &str,
    read: impl Fn(&str) -> Result<String, Fault>,
    resolve: impl Fn(&str, &str) -> Result<String, Fault>,
    target: Target,
) -> Result<Artifact, Fault> {
    let entry = Module::new(entry).map_err(fault)?;
    let mut linker = Linker::new(Reader(read), Paths(resolve), Templates);
    let output = linker.link(entry, target).map_err(diagnostic)?;
    jsengine::Engine::check(&output.javascript).map_err(|failure| Fault(failure.to_string()))?;
    let entry = output.graph.binding(&output.graph.entry);
    Ok(Artifact {
        javascript: output.javascript,
        entry: Some(entry),
    })
}

struct Reader<R>(R);

impl<R> Provider for Reader<R>
where
    R: Fn(&str) -> Result<String, Fault>,
{
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

impl<R> Resolve for Paths<R>
where
    R: Fn(&str, &str) -> Result<String, Fault>,
{
    fn resolve(&self, request: Import) -> Result<Module, Diagnostic> {
        let path = (self.0)(request.from.path(), &request.specifier)
            .map_err(|failure| Diagnostic::new(request.from.clone(), request.span, failure.0))?;
        Module::new(path).map_err(|failure| Diagnostic::new(request.from, request.span, failure.0))
    }
}

fn fault(failure: project::Fault) -> Fault {
    Fault(failure.to_string())
}

fn diagnostic(failure: Diagnostic) -> Fault {
    Fault(failure.to_string())
}
