pub mod shared;

#[cfg(target_os = "macos")]
pub mod metal;

#[cfg(windows)]
pub mod d3d12;
