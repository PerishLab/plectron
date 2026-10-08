use std::{collections::BTreeMap, sync::OnceLock};
use textengine::{Catalog, Face, Fault, Request, Selection, Shaper, Weight, font::Lease};

pub struct Resource {
    pub mime: String,
    pub body: Vec<u8>,
    pub script: Option<project::Output>,
}

#[derive(Clone)]
pub struct Application {
    root: std::path::PathBuf,
}

impl Application {
    pub fn new(name: &str) -> Self {
        assert!(
            !name.is_empty()
                && std::path::Path::new(name)
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
        );
        Self {
            root: root().join(name),
        }
    }

    pub fn load(&self, path: &str) -> Result<Resource, String> {
        let relative = path
            .strip_prefix('/')
            .ok_or_else(|| "fixture path must be absolute".to_string())?;
        let path = std::path::Path::new(relative);
        if path
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("fixture path escapes application".into());
        }
        let target = if relative.starts_with("fonts/") {
            root().join(path)
        } else {
            self.root.join(path)
        };
        let extension = target.extension().and_then(|part| part.to_str());
        let mime = match extension {
            Some("html") => "text/html",
            Some("css") => "text/css",
            Some("js" | "ts" | "pt") => "text/javascript",
            Some("png") => "image/png",
            Some("ttf") => "font/ttf",
            Some("txt") => "text/plain",
            _ => "application/octet-stream",
        };
        let script = if matches!(extension, Some("js" | "ts" | "pt")) {
            let mut workspace = project::Workspace::open(
                &target,
                project::Adapters::new(typescript::Typescript, pt::Templates),
            )
            .map_err(|failure| failure.0)?;
            Some(
                workspace
                    .link(project::Target::Script)
                    .map_err(|failure| failure.to_string())?,
            )
        } else {
            None
        };
        let body = match &script {
            Some(script) => script.javascript.as_bytes().to_vec(),
            None => std::fs::read(target).map_err(|failure| failure.to_string())?,
        };
        Ok(Resource {
            mime: mime.into(),
            body,
            script,
        })
    }
}

pub fn script(names: &str, source: &str) -> String {
    let source = format!("import {{ {names} }} from \"@plectron/engine\";\n{source}");
    let hash = source.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let entry = format!("fixture_{hash:x}.js");
    let engine = std::fs::read_to_string(root().join("../packages/engine/src/index.js"))
        .expect("engine fixture");
    let entry = project::Module::new(entry).expect("fixture entry");
    let engine_module =
        project::Module::new("packages/engine/src/index.js").expect("engine module");
    let memory = project::Memory::new([(entry.clone(), source), (engine_module, engine)]);
    let package = project::Package::parse(
        project::Module::new("packages/engine").expect("engine package"),
        r#"{"name":"@plectron/engine","exports":"./src/index.js"}"#,
    )
    .expect("engine manifest");
    let resolver = project::Resolver::new([package]).expect("fixture resolver");
    project::Linker::new(
        memory,
        resolver,
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .link(entry, project::Target::Script)
    .expect("fixture script")
    .javascript
}

pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps")
}
pub fn bytes(name: &str) -> &'static [u8] {
    static DATA: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    &DATA.get_or_init(|| {
        names()
            .iter()
            .map(|(_, _, file)| {
                (
                    (*file).to_string(),
                    std::fs::read(root().join("fonts").join(file)).expect("example font"),
                )
            })
            .collect()
    })[name]
}
pub fn face(family: &str, weight: Weight) -> Face {
    static FACES: OnceLock<BTreeMap<String, Lease>> = OnceLock::new();
    let faces = FACES.get_or_init(|| {
        names()
            .iter()
            .map(|(_, _, file)| {
                (
                    (*file).to_string(),
                    Lease::new(bytes(file).into()).expect("example font"),
                )
            })
            .collect()
    });
    let file = names()
        .into_iter()
        .find(|(name, variant, _)| *name == family && *variant == weight)
        .expect("example face")
        .2;
    Face::Owned(faces[file].clone())
}
pub fn catalog() -> Catalog {
    let catalog = Catalog::new();
    catalog.register("toy", Weight::Normal, Face::Toy).unwrap();
    catalog.register("toy", Weight::Bold, Face::Toy).unwrap();
    for (name, weight, _) in names() {
        catalog.register(name, weight, face(name, weight)).unwrap();
    }
    catalog
}
pub struct Text;
impl Shaper for Text {
    fn revision(&self) -> u64 {
        1
    }
    fn select(
        &self,
        source: &str,
        request: &Request,
        spacing: usize,
    ) -> Result<Vec<Selection>, Fault> {
        static CATALOG: OnceLock<Catalog> = OnceLock::new();
        CATALOG
            .get_or_init(catalog)
            .select(source, request, spacing)
    }
}
fn names() -> [(&'static str, Weight, &'static str); 6] {
    [
        ("lato", Weight::Normal, "Lato-Regular.ttf"),
        ("lato", Weight::Bold, "Lato-Bold.ttf"),
        ("noto-arabic", Weight::Normal, "Arabic.ttf"),
        ("noto-sc", Weight::Normal, "SC.ttf"),
        ("noto-jp", Weight::Normal, "JP.ttf"),
        ("noto-kr", Weight::Normal, "KR.ttf"),
    ]
}

pub fn stylesheet(source: &[u8]) -> Vec<u8> {
    let Ok(source) = std::str::from_utf8(source) else {
        return source.to_vec();
    };
    if source.contains("@font-face") {
        return source.as_bytes().to_vec();
    }
    let mut css = String::new();
    for (family, weight, file) in names() {
        if source.contains(family) {
            let weight = if weight == Weight::Bold {
                "bold"
            } else {
                "normal"
            };
            css.push_str(&format!("@font-face {{font-family: {family}; font-weight: {weight}; src: url(/fonts/{file});}}\n"));
        }
    }
    css.push_str(source);
    css.into_bytes()
}
