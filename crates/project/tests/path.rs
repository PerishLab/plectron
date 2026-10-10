use project::Module;
use std::path::Path;

#[test]
fn native() {
    let path = Path::new("apps").join("含 空格").join("nested/main.ts");
    assert_eq!(
        Module::file(path).unwrap().path(),
        "apps/含 空格/nested/main.ts"
    );
    assert!(Module::new("apps\\article\\main.ts").is_err());
}

#[test]
fn invalid() {
    for path in ["", "../outside.ts", "apps/../main.ts", "/apps/main.ts"] {
        assert!(Module::file(path).is_err(), "{path}");
    }
}

#[cfg(unix)]
#[test]
fn encoding() {
    use std::os::unix::ffi::OsStringExt;
    let name = std::ffi::OsString::from_vec(vec![255]);
    assert!(Module::file(Path::new("apps").join(name)).is_err());
}
