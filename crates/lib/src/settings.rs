use crate::contract::Fault;
use plumb::config::Cascade;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
const RENDERER: &str = "metal";
#[cfg(not(target_os = "macos"))]
const RENDERER: &str = "cpu";

#[derive(Debug, Cascade)]
pub struct Settings {
    #[cascade(name = "control_dir")]
    pub control: PathBuf,
    pub renderer: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            control: plumb::config::home()
                .map(|home| home.join(".local/state/plectron"))
                .unwrap_or_default(),
            renderer: RENDERER.into(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, Fault> {
        let file = plumb::config::data("plectron")
            .map(|home| home.join("config.toml"))
            .filter(|path| path.is_file());
        let settings = Self::resolve(file.as_deref()).map_err(|error| Fault(error.to_string()))?;
        if settings.control.as_os_str().is_empty() {
            return Err(Fault(format!(
                "HOME or {}_CONTROL_DIR is required",
                Self::prefix()
            )));
        }
        Ok(settings)
    }
}
