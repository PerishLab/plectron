use crate::shared::fonts::{Discovery, Fonts, Tone};

#[derive(Default)]
pub struct System {
    #[cfg(windows)]
    inner: crate::windows::fonts::Fonts,
}

impl System {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Fonts<String> for System {
    fn discover(&self, family: &str, tone: Tone) -> Result<Discovery, String> {
        #[cfg(windows)]
        {
            self.inner
                .discover(family, tone)
                .map_err(|error| error.to_string())
        }
        #[cfg(not(windows))]
        {
            let _ = (family, tone);
            Ok(Discovery::Unsupported)
        }
    }

    fn defaults(&self) -> &'static [&'static str] {
        #[cfg(windows)]
        {
            self.inner.defaults()
        }
        #[cfg(not(windows))]
        {
            &[]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform() {
        let system = System::new();
        let result = system.discover("Segoe UI", Tone::Normal).unwrap();
        #[cfg(windows)]
        assert!(matches!(result, Discovery::Found(_)));
        #[cfg(not(windows))]
        assert_eq!(result, Discovery::Unsupported);
    }
}
