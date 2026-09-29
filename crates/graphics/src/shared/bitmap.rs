use crate::shared::{Fault, Image, Item, Rect, Scene};

impl Scene {
    pub fn bitmap(frame: &Image) -> Result<Self, Fault> {
        if frame.pixels.len() != frame.extent.area()? {
            return Err(Fault("invalid bitmap extent".into()));
        }
        let first = frame.pixels[0];
        if frame.pixels.iter().all(|pixel| *pixel == first) {
            return Self::new(
                frame.extent,
                0,
                vec![Item::Fill {
                    rect: Rect {
                        x: 0,
                        y: 0,
                        width: frame.extent.width,
                        height: frame.extent.height,
                    },
                    color: Some(first),
                }],
            );
        }
        let mut items = Vec::new();
        for (y, row) in frame.pixels.chunks(frame.extent.width).enumerate() {
            let mut x = 0;
            while x < row.len() {
                let width = row[x..]
                    .iter()
                    .take_while(|pixel| **pixel == row[x])
                    .count();
                items.push(Item::Fill {
                    rect: Rect {
                        x,
                        y,
                        width,
                        height: 1,
                    },
                    color: Some(row[x]),
                });
                if items.len() > 65_536 {
                    return Err(Fault("bitmap drawing budget exhausted".into()));
                }
                x += width;
            }
        }
        Self::new(frame.extent, 0, items)
    }
}
