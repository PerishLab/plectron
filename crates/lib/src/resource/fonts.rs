use ::platform::shared::fonts::{Discovery, Fonts, Tone};
use std::collections::BTreeMap;
use std::sync::Mutex;
use textengine::font::Lease;
use textengine::{Face, Fault, Resolver, Weight};

#[derive(Default)]
pub struct System {
    host: ::platform::fonts::System,
    cache: Mutex<BTreeMap<(String, Weight), Option<Face>>>,
}

impl System {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Resolver for System {
    fn resolve(&self, family: &str, weight: Weight) -> Result<Option<Face>, Fault> {
        let key = (family.to_lowercase(), weight);
        if let Some(cached) = self.cache.lock().expect("system font cache lock").get(&key) {
            return Ok(cached.clone());
        }
        let tone = match weight {
            Weight::Normal => Tone::Normal,
            Weight::Bold => Tone::Bold,
        };
        let discovered = self.host.discover(family, tone).map_err(Fault)?;
        let face = match discovered {
            Discovery::Found(asset) => Some(Face::Owned(Lease::face(asset.bytes, asset.index)?)),
            Discovery::Absent | Discovery::Unsupported => None,
        };
        let mut cache = self.cache.lock().expect("system font cache lock");
        if cache.len() < 128 {
            cache.insert(key, face.clone());
        }
        Ok(face)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn unsupported() {
        let system = System::new();
        assert_eq!(system.resolve("Segoe UI", Weight::Normal).unwrap(), None);
    }

    #[cfg(windows)]
    mod windows {
        use super::*;
        use graphics::shared::{Backend, Cpu};
        use std::sync::Arc;
        use webengine::Page;
        use webengine::resource::{Provider, Request, Response};

        struct Entry {
            mime: String,
            body: Vec<u8>,
        }

        struct Memory {
            entries: BTreeMap<String, Entry>,
            fonts: Arc<dyn Resolver>,
        }

        impl Provider for Memory {
            fn request(&mut self, request: &Request) -> Result<Response, webengine::Fault> {
                let Some(entry) = self.entries.get(request.url.as_str()) else {
                    return Ok(Response {
                        status: 404,
                        mime: "text/plain".into(),
                        body: Vec::new(),
                    });
                };
                Ok(Response {
                    status: 200,
                    mime: entry.mime.clone(),
                    body: entry.body.clone(),
                })
            }
            fn fonts(&self) -> Option<Arc<dyn Resolver>> {
                Some(self.fonts.clone())
            }
        }

        #[test]
        fn mixed() {
            let system = Arc::new(System::new());
            let catalog = textengine::Catalog::new();
            catalog.resolver(Some(system.clone()));
            let request = textengine::Request {
                families: Arc::from(["Segoe UI".into(), "Microsoft YaHei".into()]),
                size: 24.into(),
                weight: Weight::Normal,
            };
            let shaped = textengine::Shaper::shape(&catalog, "Hello 世界", &request, 0).unwrap();
            assert!(shaped.issues.is_empty(), "{:?}", shaped.issues);
            assert!(shaped.spans.len() >= 2);
            for span in &shaped.spans {
                assert!(matches!(span.font.face, Face::Owned(_)));
            }
            let html = r#"<!doctype html><html><head><link rel="stylesheet" href="app.css"></head><body><div class="box">Hello 世界 Plectron</div></body></html>"#;
            let css = r#"body { margin: 0; background-color: #ffffff; } .box { font-family: "Segoe UI", "Microsoft YaHei", sans-serif; font-size: 24px; color: #111111; padding: 16px; }"#;
            let mut provider = Memory {
                entries: BTreeMap::from([
                    (
                        "app://test/index.html".into(),
                        Entry {
                            mime: "text/html".into(),
                            body: html.as_bytes().to_vec(),
                        },
                    ),
                    (
                        "app://test/app.css".into(),
                        Entry {
                            mime: "text/css".into(),
                            body: css.as_bytes().to_vec(),
                        },
                    ),
                ]),
                fonts: system,
            };
            let mut page = Page::new(
                "app://test/index.html",
                domengine::Extent {
                    width: 480,
                    height: 120,
                },
            )
            .unwrap();
            page.load(&mut provider).unwrap();
            assert_eq!(page.loading().fonts, 0);
            let frame = page.frame().expect("rendered frame");
            let image = Cpu.render(&frame).unwrap();
            let ink = image
                .pixels
                .iter()
                .filter(|pixel| **pixel != 0x00ff_ffff)
                .count();
            assert!(
                ink > 200,
                "expected crisp rendered Latin+CJK ink, got {ink}"
            );
            let chinese = r#"<!doctype html><html><head><link rel="stylesheet" href="app.css"></head><body><div class="box">中文渲染测试</div></body></html>"#;
            provider.entries.insert(
                "app://test/cjk.html".into(),
                Entry {
                    mime: "text/html".into(),
                    body: chinese.as_bytes().to_vec(),
                },
            );
            let mut cjk = Page::new(
                "app://test/cjk.html",
                domengine::Extent {
                    width: 320,
                    height: 120,
                },
            )
            .unwrap();
            cjk.load(&mut provider).unwrap();
            assert_eq!(cjk.loading().fonts, 0);
            let pixels = Cpu.render(&cjk.frame().expect("CJK frame")).unwrap().pixels;
            assert!(
                pixels.iter().filter(|pixel| **pixel != 0x00ff_ffff).count() > 200,
                "expected crisp rendered CJK ink from host msyh.ttc"
            );
        }
    }
}
