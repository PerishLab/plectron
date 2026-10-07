#[test]
fn lifecycle() {
    let mut page = super::loaded(&mut super::source());
    assert_eq!(
        page.evaluate("button === document.getElementById('button');")
            .unwrap(),
        "true"
    );
    page.evaluate("const parent = document.getElementById('body'); parent.removeChild(button); parent.appendChild(button);").unwrap();
    assert_eq!(
        page.evaluate("button === document.getElementById('button');")
            .unwrap(),
        "true"
    );
    assert!(
        page.evaluate("parent.appendChild({});")
            .unwrap_err()
            .0
            .contains("expected a node")
    );
    page.render().unwrap();
    page.close();
    assert!(page.evaluate("parent.appendChild(button);").is_err());
}
