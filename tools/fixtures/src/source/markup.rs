use project::{Import, Module, Resolve, Resolver, Span};

pub fn scripts(module: &Module, source: &str) -> Result<Vec<Module>, String> {
    let mut entries = Vec::new();
    for chunk in source.split("<script").skip(1) {
        let (attributes, _) = chunk
            .split_once('>')
            .ok_or("incomplete script declaration")?;
        let value = attributes
            .trim()
            .strip_prefix("src")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
            .ok_or("script declaration requires an explicit source")?
            .trim();
        let quote = value
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
            .ok_or("script source must be quoted")?;
        let (path, rest) = value[1..]
            .split_once(quote)
            .ok_or("incomplete script source")?;
        if path.is_empty() || !rest.trim().is_empty() {
            return Err("unsupported maintained script declaration".into());
        }
        let specifier = if path.starts_with('.') {
            path.into()
        } else {
            format!("./{path}")
        };
        let entry = Resolver::default()
            .resolve(Import {
                from: module.clone(),
                specifier,
                span: Span::default(),
            })
            .map_err(|error| error.to_string())?;
        match super::role(entry.path())? {
            super::Role::Canonical | super::Role::Driver => {}
            _ => return Err(format!("nonapplication script entry: {entry}")),
        }
        if entries.contains(&entry) {
            return Err(format!("duplicate content script: {entry}"));
        }
        entries.push(entry);
    }
    Ok(entries)
}
