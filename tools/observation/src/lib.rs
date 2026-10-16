use plumb::observation::{Declaration, Run};

pub fn start(command: &str) -> Option<Run> {
    let home = plumb::config::data("plectron")?;
    let file = home.join("config.toml");
    let declaration = Declaration {
        producer: "plectron",
        file: &file,
        buffer: home.join("state/locus"),
    };
    Run::start(declaration, command)
}

pub const HELP: &str = "Locus CLI observation is disabled by default. Set [locus] enabled = true in config.toml under ~/.plectron on Unix or LOCALAPPDATA/plectron on Windows. Buffers use state/locus there. endpoint overrides LOCUS_API, then http://127.0.0.1:43308. Observation failures preserve command output and exit status; native runtime observation remains separate.";
