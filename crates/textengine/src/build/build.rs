mod breaking;
mod grapheme;

fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let unicode = root.join("resources/unicode");
    for name in [
        "LineBreak.txt",
        "EastAsianWidth.txt",
        "DerivedGeneralCategory.txt",
        "GraphemeBreakProperty.txt",
        "DerivedCoreProperties.txt",
        "emoji-data.txt",
    ] {
        println!("cargo:rerun-if-changed={}", unicode.join(name).display());
    }
    breaking::generate(&unicode, &output).unwrap();
    grapheme::generate(&unicode, &output).unwrap();
}
