mod projection;
pub(in crate::layout) mod tree;

pub use geoengine::{Delta as Layout, Scan as Survey};
pub use projection::Projection;
pub use tree::Tree;

#[derive(Clone, Copy)]
pub(in crate::layout) enum Transition {
    Translate([i64; 2]),
    Opacity(u8),
    Composite { translation: [i64; 2], opacity: u8 },
}
