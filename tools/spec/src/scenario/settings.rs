use crate::Result;

pub(super) struct Settings {
    pub count: usize,
    pub runs: usize,
    pub edits: usize,
    pub reference: Option<u64>,
    pub warmups: usize,
    pub retained: bool,
}
impl Settings {
    pub fn read(config: &config::Store, defaults: (usize, usize)) -> Result<Self> {
        let settings = Self {
            count: value(config, "scenario.count")?.unwrap_or(defaults.0),
            runs: value(config, "scenario.runs")?.unwrap_or(defaults.1),
            edits: value(config, "scenario.edits")?.unwrap_or(1),
            reference: value(config, "scenario.reference")?,
            warmups: value(config, "scenario.warmups")?.unwrap_or(0),
            retained: value(config, "scenario.retained")?.unwrap_or(false),
        };
        if !(2..=4096).contains(&settings.count) || settings.runs == 0 {
            return Err("invalid scenario count or runs".into());
        }
        if !(1..=settings.count + settings.count.div_ceil(4)).contains(&settings.edits) {
            return Err("invalid scenario edit range".into());
        }
        Ok(settings)
    }
}
pub(super) fn viewport(case: &mut super::Case, config: &config::Store) -> Result<()> {
    if let Some(viewport) = value(config, "scenario.viewport")? {
        case.viewport = viewport;
    }
    graphics::shared::Extent {
        width: case.viewport[0],
        height: case.viewport[1],
    }
    .area()
    .map_err(|error| error.to_string())?;
    Ok(())
}
fn value<T: serde::de::DeserializeOwned>(config: &config::Store, key: &str) -> Result<Option<T>> {
    config
        .get(key)
        .map(|value| serde_json::from_value(value).map_err(|error| format!("{key}: {error}")))
        .transpose()
}
pub(super) fn prepare(
    page: &mut webengine::Page,
    config: &config::Store,
    region: &str,
) -> std::result::Result<(), webengine::Fault> {
    apply(page, config)?;
    if let Some(height) = value::<usize>(config, "scenario.height").map_err(webengine::Fault)? {
        let region = serde_json::to_string(region).map_err(|e| webengine::Fault(e.to_string()))?;
        page.evaluate(&format!(
            "document.getElementById({region}).setAttribute(\"style\", \"height: {height}px;\");"
        ))?;
    }
    Ok(())
}

pub(super) fn apply(
    page: &mut webengine::Page,
    config: &config::Store,
) -> std::result::Result<(), webengine::Fault> {
    for cache in page.inspect().caches {
        if let Some(value) = config.get(&format!("scenario.cache.{}", cache.name)) {
            let policy = serde_json::from_value(value)
                .map_err(|error| webengine::Fault(error.to_string()))?;
            page.cache(cache.id, shared::cache::Command::Configure { policy })?;
        }
    }
    Ok(())
}
