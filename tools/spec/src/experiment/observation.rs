use std::path::{Path, PathBuf};

pub fn run(
    paths: (&Path, &Path),
    platform: (&'static str, &'static str),
    executable: PathBuf,
    options: observe::Options,
) -> crate::Result<crate::experiment::Manifest> {
    let session = observe::Session::new(options);
    let mut root = session.scope("experiment");
    let result = crate::experiment::run(paths.0, paths.1, platform, executable);
    root.finish(result.as_ref().is_ok_and(|report| report.valid()));
    session.stop();
    if paths.1.is_dir() {
        let file = std::fs::File::create_new(paths.1.join("observation.json"))
            .map_err(|e| e.to_string())?;
        serde_json::to_writer(file, &session.inspect()).map_err(|e| e.to_string())?;
    }
    result
}
