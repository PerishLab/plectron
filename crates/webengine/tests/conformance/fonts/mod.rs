mod loading;
use super::*;
use graphics::shared::{Backend, Cpu};
use std::sync::Arc;
use textengine::{Face, Weight, font::Lease};

#[test]
fn lifecycle() {
    let mut source = source();
    source.files.get_mut("/site/style.css").unwrap().body =
        "body {font-family: custom, toy; font-size: 24px;}".into();
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 240,
            height: 100,
        },
    )
    .unwrap();
    let fonts = page.fonts();
    let face = Lease::new(Arc::from(::fixtures::bytes("Lato-Regular.ttf"))).unwrap();
    let registration = fonts
        .register("custom", Weight::Normal, Face::Owned(face))
        .unwrap();
    page.load(&mut source).unwrap();
    let original = page.frame().unwrap();
    let pixels = Cpu.render(&original).unwrap();
    page.acknowledge(original.id()).unwrap();
    let revision = page.inspect().revision;
    assert!(fonts.unregister(&registration).unwrap());
    page.render().unwrap();
    assert_eq!(page.inspect().revision, revision);
    let changed = page.frame().unwrap();
    assert_ne!(Cpu.render(&changed).unwrap(), pixels);
    page.acknowledge(changed.id()).unwrap();
    page.render().unwrap();
    assert!(page.frame().is_none());
    fonts.close();
    assert!(page.render().is_err());
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
    page.close();
    assert!(fonts.register("late", Weight::Normal, Face::Toy).is_err());
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}
