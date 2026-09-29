use crate::{Diagnostic, Fault, Import, Module, Package, Resolve};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Resolver {
    packages: BTreeMap<String, Package>,
}

impl Resolver {
    pub fn new(packages: impl IntoIterator<Item = Package>) -> Result<Self, Fault> {
        let mut resolver = Self::default();
        for package in packages {
            if resolver
                .packages
                .insert(package.name.clone(), package)
                .is_some()
            {
                return Err(Fault("duplicate package declaration".into()));
            }
        }
        Ok(resolver)
    }
}

impl Resolve for Resolver {
    fn resolve(&self, request: Import) -> Result<Module, Diagnostic> {
        if request.specifier.starts_with('.') {
            return relative(&request.from, &request.specifier)
                .map_err(|fault| diagnostic(request, fault));
        }
        bare(&self.packages, &request.specifier).map_err(|fault| diagnostic(request, fault))
    }
}

fn relative(from: &Module, specifier: &str) -> Result<Module, Fault> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return Err(Fault(format!("invalid relative specifier: {specifier}")));
    }
    let mut parts = from.path().split('/').collect::<Vec<_>>();
    parts.pop();
    for part in specifier.split('/') {
        match part {
            "." => {}
            ".." => retreat(&mut parts, specifier)?,
            "" => return Err(Fault(format!("invalid module specifier: {specifier}"))),
            part => parts.push(part),
        }
    }
    let module = Module::new(parts.join("/"))?;
    module.kind()?;
    Ok(module)
}

fn retreat(parts: &mut Vec<&str>, specifier: &str) -> Result<(), Fault> {
    parts
        .pop()
        .map(|_| ())
        .ok_or_else(|| Fault(format!("module leaves project boundary: {specifier}")))
}

fn bare(packages: &BTreeMap<String, Package>, specifier: &str) -> Result<Module, Fault> {
    let (name, subpath) = split(specifier)?;
    let package = packages
        .get(name)
        .ok_or_else(|| Fault(format!("undeclared package: {name}")))?;
    let key = subpath.map_or_else(|| ".".to_string(), |path| format!("./{path}"));
    package
        .exports
        .get(&key)
        .cloned()
        .ok_or_else(|| Fault(format!("unknown package export: {specifier}")))
}

fn split(specifier: &str) -> Result<(&str, Option<&str>), Fault> {
    if specifier.is_empty() || specifier.starts_with('/') {
        return Err(Fault(format!("invalid package specifier: {specifier}")));
    }
    if specifier.starts_with('@') {
        let mut parts = specifier.splitn(3, '/');
        let scope = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        if scope.len() == 1 || name.is_empty() {
            return Err(Fault(format!("invalid package specifier: {specifier}")));
        }
        let length = scope.len() + name.len() + 1;
        Ok((&specifier[..length], parts.next()))
    } else {
        Ok(specifier
            .split_once('/')
            .map_or((specifier, None), |value| (value.0, Some(value.1))))
    }
}

fn diagnostic(request: Import, fault: Fault) -> Diagnostic {
    Diagnostic::new(request.from, request.span, fault.0)
}
