pub mod benchmark;
pub mod budget;
pub mod experiment;
pub mod harness;
pub mod orchestration;
mod profile;
pub mod scenario;
pub mod suite;
pub mod typography;

pub use profile::{Area, Capability, Case, Mode, Phase, Profile, Reason, Selection};

pub type Result<T> = std::result::Result<T, String>;

pub fn root() -> std::path::PathBuf {
    fixtures::root()
        .parent()
        .expect("application repository")
        .to_owned()
}

pub(crate) fn resource(name: &str) -> Result<String> {
    let path = root().join("tools/spec/resources").join(name);
    std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
}
