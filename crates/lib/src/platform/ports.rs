use crate::contract::Fault;
use std::sync::Arc;

pub use ::platform::shared::{Clipboard, Console, Display, Input};

pub trait Backend: ::platform::shared::Backend<Fault> {}
impl<T: ::platform::shared::Backend<Fault>> Backend for T {}

struct Bridge(Arc<dyn Clipboard>);

impl webengine::keyboard::Port for Bridge {
    fn read(&self) -> Result<Option<String>, Fault> {
        self.0.read().map_err(Fault)
    }

    fn write(&self, text: &str) -> Result<(), Fault> {
        self.0.write(text).map_err(Fault)
    }
}

pub(super) fn clipboard(port: Arc<dyn Clipboard>) -> Arc<dyn webengine::keyboard::Port> {
    Arc::new(Bridge(port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contract::{Extent, Window},
        platform::{Host, Platform},
        rendering::Solid,
    };
    use std::sync::Mutex;

    struct Store(Mutex<Option<String>>);

    impl Clipboard for Store {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn write(&self, text: &str) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(text.into());
            Ok(())
        }
    }

    struct Stub(Arc<Store>);

    impl Display<Fault> for Stub {
        fn create(&mut self, _: crate::contract::Unit, _: &Window) -> Result<(), Fault> {
            Ok(())
        }
        fn title(&mut self, _: crate::contract::Unit, _: &str) -> Result<(), Fault> {
            Ok(())
        }
        fn present(
            &mut self,
            _: crate::contract::Unit,
            _: ::platform::shared::Image,
        ) -> Result<(), Fault> {
            Ok(())
        }
        fn close(&mut self, _: crate::contract::Unit) -> Result<(), Fault> {
            Ok(())
        }
    }

    impl Input<Fault> for Stub {
        fn poll(&mut self) -> Result<Vec<::platform::shared::Signal>, Fault> {
            Ok(Vec::new())
        }
        fn clipboard(&self) -> Option<Arc<dyn Clipboard>> {
            Some(self.0.clone())
        }
    }

    impl Console<Fault> for Stub {
        fn log(&mut self, _: &str) -> Result<(), Fault> {
            Ok(())
        }
    }

    #[test]
    fn wiring() {
        let store = Arc::new(Store(Mutex::new(Some("hello".into()))));
        let mut platform = Platform::new(Stub(store.clone()), Solid);
        let root = fixtures::root();
        let service = platform.serve(root.to_str().unwrap()).unwrap();
        platform.scheme(service, "app://clip/").unwrap();
        let unit = platform
            .window(Window::new(
                "clip",
                Extent {
                    width: 200,
                    height: 120,
                },
            ))
            .unwrap();
        platform
            .attach(unit, "app://clip/site/src/index.html")
            .unwrap();
        platform.finish().unwrap();
        platform
            .evaluate(
                unit,
                "const field = document.createElement('input'); field.setAttribute('id', 'field'); document.getElementById('page').appendChild(field); field.focus();",
            )
            .unwrap();
        let paste = shared::shortcut::Input::Key(shared::shortcut::Stroke {
            chord: shared::shortcut::Chord::parse("Primary+KeyV").unwrap(),
            repeat: false,
            text: String::new(),
        });
        platform
            .input(&crate::contract::Signal {
                unit,
                event: crate::contract::Event::Input(paste),
            })
            .unwrap();
        let read = "document.getElementById('field').getValue();";
        let super::super::Evaluation::Value(value) = platform.evaluate(unit, read).unwrap() else {
            panic!("expected local evaluation");
        };
        assert_eq!(value, "hello");
        platform
            .evaluate(unit, "document.getElementById('field').setSelection(1, 4);")
            .unwrap();
        let copy = shared::shortcut::Input::Key(shared::shortcut::Stroke {
            chord: shared::shortcut::Chord::parse("Primary+KeyC").unwrap(),
            repeat: false,
            text: String::new(),
        });
        platform
            .input(&crate::contract::Signal {
                unit,
                event: crate::contract::Event::Input(copy),
            })
            .unwrap();
        assert_eq!(store.0.lock().unwrap().as_deref(), Some("ell"));
        let cut = shared::shortcut::Input::Key(shared::shortcut::Stroke {
            chord: shared::shortcut::Chord::parse("Primary+KeyX").unwrap(),
            repeat: false,
            text: String::new(),
        });
        platform
            .input(&crate::contract::Signal {
                unit,
                event: crate::contract::Event::Input(cut),
            })
            .unwrap();
        let super::super::Evaluation::Value(after) = platform.evaluate(unit, read).unwrap() else {
            panic!("expected local evaluation");
        };
        assert_eq!(after, "ho");
        assert_eq!(store.0.lock().unwrap().as_deref(), Some("ell"));
    }
}
