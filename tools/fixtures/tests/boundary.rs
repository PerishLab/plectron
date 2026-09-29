use std::path::{Path, PathBuf};

fn files(root: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path
            .file_name()
            .is_some_and(|name| name == "target" || name == ".git" || name == "node_modules")
        {
            continue;
        }
        if path.is_dir() {
            files(&path, found);
        } else if path
            .extension()
            .is_some_and(|value| value == "rs" || value == "js")
        {
            found.push(path);
        }
    }
}

fn embedded(path: &Path, source: &str, name: &str, applications: &str) {
    for (offset, _) in source.match_indices(name) {
        let end = source.len().min(offset + 256);
        assert!(
            !source[offset..end].contains(applications),
            "embedded application source in {}",
            path.display()
        );
    }
}

#[test]
fn boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    files(&root, &mut paths);
    let private = ["__", "plectron"].concat();
    let includes = [
        ["include_", "str!"].concat(),
        ["include_", "bytes!"].concat(),
    ];
    let applications = ["app", "s/"].concat();
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains(&private),
            "legacy addon registry in {}",
            path.display()
        );
        if path.components().any(|part| part.as_os_str() == "tests") {
            for name in &includes {
                embedded(&path, &source, name, &applications);
            }
        }
    }
}
