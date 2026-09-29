use crate::shared::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Area {
    #[serde(deserialize_with = "coordinate")]
    pub x: i128,
    #[serde(deserialize_with = "coordinate")]
    pub y: i128,
    pub width: usize,
    pub height: usize,
}

fn coordinate<'de, D: serde::Deserializer<'de>>(input: D) -> Result<i128, D::Error> {
    struct Visitor;
    impl serde::de::Visitor<'_> for Visitor {
        type Value = i128;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an integer coordinate")
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<i128, E> {
            Ok(i128::from(value))
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<i128, E> {
            Ok(i128::from(value))
        }
        fn visit_i128<E: serde::de::Error>(self, value: i128) -> Result<i128, E> {
            Ok(value)
        }
    }
    input.deserialize_any(Visitor)
}
impl From<Rect> for Area {
    fn from(rect: Rect) -> Self {
        Self {
            x: rect.x as i128,
            y: rect.y as i128,
            width: rect.width,
            height: rect.height,
        }
    }
}
impl Area {
    pub fn relative(rect: Rect, origin: [usize; 2]) -> Self {
        Self {
            x: rect.x as i128 - origin[0] as i128,
            y: rect.y as i128 - origin[1] as i128,
            width: rect.width,
            height: rect.height,
        }
    }
    pub fn clip(self, clip: Option<Self>) -> Self {
        let Some(clip) = clip else {
            return self;
        };
        let x = self.x.max(clip.x);
        let y = self.y.max(clip.y);
        let right = (self.x + self.width as i128).min(clip.x + clip.width as i128);
        let bottom = (self.y + self.height as i128).min(clip.y + clip.height as i128);
        Self {
            x,
            y,
            width: (right - x).max(0) as usize,
            height: (bottom - y).max(0) as usize,
        }
    }
    pub fn map(self, delta: [i64; 2]) -> Rect {
        let x = self.x + i128::from(delta[0]);
        let y = self.y + i128::from(delta[1]);
        let right = (x + self.width as i128).clamp(0, usize::MAX as i128);
        let bottom = (y + self.height as i128).clamp(0, usize::MAX as i128);
        let left = x.clamp(0, usize::MAX as i128);
        let top = y.clamp(0, usize::MAX as i128);
        Rect {
            x: left as usize,
            y: top as usize,
            width: (right - left) as usize,
            height: (bottom - top) as usize,
        }
    }
}
