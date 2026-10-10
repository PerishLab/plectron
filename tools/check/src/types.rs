use crate::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct Projection {
    directory: tempfile::TempDir,
    pub templates: usize,
}

impl Projection {
    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    pub fn config(&self) -> PathBuf {
        self.root().join("tsconfig.json")
    }
}

pub fn project(root: &Path) -> Result<Projection> {
    let root = root.canonicalize().map_err(error)?;
    let directory = tempfile::tempdir().map_err(error)?;
    let mut templates = 0;
    for name in ["apps", "packages", "types"] {
        copy(
            &root.join(name),
            &directory.path().join(name),
            &mut templates,
        )?;
    }
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(root.join("tsconfig.json")).map_err(error)?)
            .map_err(error)?;
    let options = config["compilerOptions"]
        .as_object_mut()
        .ok_or("tsconfig requires compilerOptions")?;
    options.insert("allowArbitraryExtensions".into(), json!(true));
    options.insert(
        "typeRoots".into(),
        json!([root.join("node_modules/@types")]),
    );
    let aliases = options
        .entry("paths")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("tsconfig paths must be an object")?;
    for entry in std::fs::read_dir(root.join("packages")).map_err(error)? {
        let entry = entry.map_err(error)?.path();
        if !entry.is_dir() {
            continue;
        }
        let relative = entry
            .strip_prefix(&root)
            .map_err(error)?
            .to_str()
            .ok_or("package path is not text")?;
        let manifest = std::fs::read_to_string(entry.join("package.json")).map_err(error)?;
        let package =
            project::Package::parse(project::Module::new(relative).map_err(error)?, &manifest)
                .map_err(error)?;
        for (key, module) in package.exports {
            let alias = if key == "." {
                package.name.clone()
            } else {
                format!(
                    "{}/{}",
                    package.name,
                    key.strip_prefix("./").ok_or("invalid package export")?
                )
            };
            aliases.insert(alias, json!([directory.path().join(module.path())]));
        }
    }
    std::fs::write(
        directory.path().join("tsconfig.json"),
        serde_json::to_vec_pretty(&config).map_err(error)?,
    )
    .map_err(error)?;
    Ok(Projection {
        directory,
        templates,
    })
}

pub fn check(root: &Path) -> Result<()> {
    let projection = project(root)?;
    eprintln!(
        "typecheck: {} compiler-derived PT declarations",
        projection.templates
    );
    let status = crate::command(root, "pnpm")?
        .args(["exec", "tsc", "--noEmit", "--project"])
        .arg(projection.config())
        .status()
        .map_err(error)?;
    if !status.success() {
        return Err("template-aware typecheck failed".into());
    }
    Ok(())
}

fn copy(source: &Path, target: &Path, templates: &mut usize) -> Result<()> {
    std::fs::create_dir_all(target).map_err(error)?;
    for entry in std::fs::read_dir(source).map_err(error)? {
        let path = entry.map_err(error)?.path();
        let name = path.file_name().ok_or("source has no file name")?;
        if name.to_str().is_some_and(|name| name.ends_with(".d.pt.ts")) {
            return Err(format!("parallel template declaration: {}", path.display()));
        }
        if path.is_dir() {
            if !matches!(
                name.to_str(),
                Some("node_modules" | "dist" | "target" | ".git")
            ) {
                copy(&path, &target.join(name), templates)?;
            }
            continue;
        }
        match path.extension().and_then(|value| value.to_str()) {
            Some("pt") => {
                let text = std::fs::read_to_string(&path).map_err(error)?;
                let shape = pt::compile(&text)
                    .map_err(|failure| format!("{}: {failure}", path.display()))?
                    .shape;
                let declaration = target.join(name).with_extension("d.pt.ts");
                std::fs::write(declaration, shape.declaration()).map_err(error)?;
                *templates += 1;
            }
            Some("ts" | "js" | "json") => {
                std::fs::copy(&path, target.join(name)).map_err(error)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
