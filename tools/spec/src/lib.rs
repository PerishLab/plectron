pub mod benchmark;
pub mod budget;
pub mod experiment;
pub mod harness;
mod profile;
pub mod scenario;
pub mod suite;
pub mod typography;

pub use profile::{Area, Capability, Case, Mode, Phase, Profile, Reason, Selection};

pub type Result<T> = std::result::Result<T, String>;

pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
