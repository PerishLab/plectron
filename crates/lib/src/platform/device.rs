pub use ::platform::shared::{Button, Composition, Event, Id, Keyboard, Phase, Point, Signal};

pub(crate) fn content(point: Point) -> Option<(usize, usize)> {
    usize::try_from(point.x)
        .ok()
        .zip(usize::try_from(point.y).ok())
}
