use super::super::{Extent, Page, source};
use config::Store;

#[test]
fn distribution() {
    let config = Store::default();
    config.set("dom.history", 4.into());
    let mut page = Page::configured(
        "app://demo/site/index.html",
        Extent {
            width: 200,
            height: 120,
        },
        config.clone(),
    )
    .unwrap();
    page.load(&mut source()).unwrap();
    let initial = page.inspect();
    assert_eq!(initial.update.history.limit, 4);
    page.evaluate("button.setText('Next');").unwrap();
    page.render().unwrap();
    assert!(page.inspect().update.reset.is_none());
    config.set("dom.history", 0.into());
    page.evaluate("button.setText('Reconstructed');").unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().update.reset.as_deref(), Some("history"));
    assert_eq!(page.inspect().update.history.bytes, 0);
    config.set("dom.history", 32.into());
    page.evaluate("button.setText('Resumed');").unwrap();
    page.render().unwrap();
    assert!(page.inspect().update.reset.is_none());
    assert_eq!(page.inspect().update.history.limit, 32);
    config.set("dom.history", "invalid".into());
    assert!(
        page.evaluate("count = 99;")
            .unwrap_err()
            .0
            .contains("dom.history")
    );
    config.remove("dom.history");
    assert_eq!(page.evaluate("count;").unwrap(), "0");
    page.evaluate("button.setText('Default');").unwrap();
    page.render().unwrap();
    assert_eq!(
        page.inspect().update.history.limit,
        domengine::Document::default().retention().limit,
    );
    assert_eq!(initial.update.history.limit, 4);
}
