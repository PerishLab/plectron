#![forbid(unsafe_code)]

mod driver;
pub use driver::{Driver, Pacing, Pulse, Receipt, Sighting, Visibility};
mod bitmap;
mod cpu;
mod fault;
mod geometry;
mod queue;
mod scene;
pub use scene::{font, wire};

pub use cpu::{Compute, Cpu, Task, raster};
pub use fault::Fault;
pub use geometry::{
    Bands, Clip, Coverage, Curve, Evaluation, Extent, Geometry, Image, Output, Path, Point, Raster,
    Rect, Sample, Scalar, Segment, Transform,
};
pub use queue::{Backend, Queue, Sink};
pub use queue::{State, Submission, Target};
pub use scene::{
    Area, Bitmap, Effect, Effects, Face, Font, Frame, Glyph, Impact, Ink, Item, Layer, Mapping,
    Materialization, Pass, Primitive, Record, Scene, Space, Spatial, Visual, Weight,
};
pub mod preparation;
pub use geometry::coverage;
pub use preparation::{Preparation, Rasterization, Rasterizer};

pub mod operation;
pub use scene::{CHECKS, Routing};
