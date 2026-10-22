fn main() {
    println!("cargo:rerun-if-changed=src/native/trace.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/native/trace.c")
            .compile("plectron_observe");
    }
}
