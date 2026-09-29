pub mod change;
mod document;
mod editing;
pub use editing::{Buffer, Direction, Edit, Projection, Selection};
pub mod ime {
    pub use crate::editing::ime::{Composition, Effect, Emphasis, Id, Input, Span};
}
mod fault;
pub mod html;
pub mod layout;
mod resource;
pub mod style;

pub use document::{Collection, Document, Entry, Kind, Node};
pub use fault::Fault;
pub use graphics::shared::{Extent, Glyph, Image, Item, Rect, raster};
pub use layout::{
    Activity, Available, Boundary, Branch, Capture, Category, Causality, Cluster, Constraints,
    Construction, Content, Coverage, Estimation, Event, Fragment, Input, Intrinsic, Layout, Link,
    Local, Operations, Painter, Painting, Piece, Pipeline, Placement, Quality, Reuse, Run, Shaper,
    Shaping, Text, Update,
};
pub use style::Sheet;

pub use layout::operation;
