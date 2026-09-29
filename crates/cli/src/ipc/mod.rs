mod client;
#[cfg(target_os = "macos")]
mod server;
mod transfer;
pub use client::{list, send};
use plectron::contract::Fault;
#[cfg(target_os = "macos")]
pub use server::Server;
pub use transfer::export;
pub fn observation(
    directory: &std::path::Path,
    target: (u32, u64),
    output: &std::path::Path,
) -> Result<(), Fault> {
    transfer::receive(
        directory,
        (
            target.0,
            plectron::ipc::Command::Observe {
                command: plectron::ipc::observation::Command::Export { id: target.1 },
            },
        ),
        output,
        ("plectron.observation", 1),
    )
}
pub fn error(error: impl std::fmt::Display) -> Fault {
    Fault(error.to_string())
}
