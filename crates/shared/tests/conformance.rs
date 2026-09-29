use shared::Resource;

#[test]
fn text() {
    const RESOURCE: Resource = Resource::new("雪\r\n\0".as_bytes());
    assert_eq!(RESOURCE.text().unwrap(), "雪\r\n\0");
    assert_eq!(RESOURCE.bytes(), &[0xe9, 0x9b, 0xaa, 13, 10, 0]);
}

#[test]
fn binary() {
    const RESOURCE: Resource = Resource::new(&[0, 0xff, 0x80]);
    let error = RESOURCE.text().unwrap_err();
    assert_eq!(error.valid_up_to(), 1);
    assert_eq!(RESOURCE.bytes(), &[0, 0xff, 0x80]);
}
