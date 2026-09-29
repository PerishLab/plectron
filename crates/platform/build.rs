fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/macos/ime/native.m");
        println!("cargo:rerun-if-changed=src/macos/window/native.m");
        cc::Build::new()
            .file("src/macos/ime/native.m")
            .file("src/macos/window/native.m")
            .flag("-fobjc-exceptions")
            .compile("plectron_macos");
    }
}
