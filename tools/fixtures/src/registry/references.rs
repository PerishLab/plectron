use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

#[derive(Default)]
pub(super) struct Facts {
    pub dependencies: BTreeSet<String>,
    pub content: BTreeSet<String>,
}

pub(super) fn inspect(root: &Path, name: &str) -> Result<Facts, String> {
    let mut result = Facts::default();
    visit(root, &root.join(name), name, &mut result)?;
    Ok(result)
}

fn visit(root: &Path, directory: &Path, name: &str, result: &mut Facts) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(error)? {
        let path = entry.map_err(error)?.path();
        if path.is_dir() {
            visit(root, &path, name, result)?;
            continue;
        }
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("html" | "css" | "ts" | "js" | "pt")) {
            continue;
        }
        let source = std::fs::read_to_string(&path).map_err(error)?;
        for (offset, reference) in literals(&source) {
            let loading = matches!(extension, Some("ts" | "js"))
                && source[..offset]
                    .trim_end()
                    .strip_suffix('(')
                    .is_some_and(|prefix| prefix.trim_end().ends_with(".load"));
            let target = if reference.starts_with("../") {
                Some(path.parent().unwrap().join(reference))
            } else if loading {
                reference
                    .strip_prefix("app://")
                    .and_then(|url| url.split_once('/'))
                    .map(|(_, path)| root.join(path))
            } else {
                None
            };
            if let Some(target) = target {
                let target = normalize(&target)?;
                if !target.is_file() {
                    return Err(format!(
                        "missing static resource: {} -> {reference}",
                        path.display()
                    ));
                }
                let relative = target.strip_prefix(normalize(root)?).map_err(error)?;
                if loading {
                    result
                        .content
                        .insert(relative.to_string_lossy().into_owned());
                }
                let owner = relative
                    .components()
                    .next()
                    .ok_or("resource has no owner")?
                    .as_os_str()
                    .to_str()
                    .ok_or("resource owner is not text")?;
                if owner != name {
                    result.dependencies.insert(owner.into());
                }
            }
        }
    }
    Ok(())
}

fn literals(source: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'"' | b'\'') {
            let start = cursor + 1;
            cursor = start;
            while cursor < source.len() && source.as_bytes()[cursor] != byte {
                if source.as_bytes()[cursor] == b'\\' {
                    cursor += 1;
                }
                cursor += 1;
            }
            if cursor < source.len() {
                result.push((start - 1, &source[start..cursor]));
            }
        }
        cursor += 1;
    }
    for (offset, _) in source.match_indices("url(") {
        let chunk = &source[offset + 4..];
        if let Some((value, _)) = chunk.split_once(')') {
            result.push((offset, value.trim().trim_matches(['"', '\''])));
        }
    }
    result
}

fn normalize(path: &Path) -> Result<PathBuf, String> {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                if !result.pop() {
                    return Err("static resource escapes root".into());
                }
            }
            Component::CurDir => {}
            part => result.push(part.as_os_str()),
        }
    }
    Ok(result)
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        let source = "window.load ( 'app://demo/site/index.html' ); url(../fonts/Lato-Regular.ttf)";
        let values = super::literals(source);
        assert_eq!(
            values.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            ["app://demo/site/index.html", "../fonts/Lato-Regular.ttf"]
        );
        let offset = values[0].0;
        assert!(
            source[..offset]
                .trim_end()
                .strip_suffix('(')
                .unwrap()
                .trim_end()
                .ends_with(".load")
        );
    }

    #[test]
    fn paths() {
        assert_eq!(
            normalize(Path::new("/apps/list/../fonts/font.ttf")).unwrap(),
            Path::new("/apps/fonts/font.ttf")
        );
        assert!(normalize(Path::new("/../font.ttf")).is_err());
    }
}
