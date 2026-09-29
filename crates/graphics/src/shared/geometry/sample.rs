use crate::shared::{Clip, Raster, Rect};

#[derive(Debug)]
pub struct Sample {
    pub raster: Raster,
    pub target: [f32; 4],
    pub clip: Clip,
}
impl Sample {
    pub fn bounds(&self) -> Rect {
        self.clip.bounds()
    }
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        let extent = self.raster.extent();
        let point = [x as f32 + 0.5, y as f32 + 0.5];
        let sizes = [extent.width, extent.height];
        let mut position = [0.0; 2];
        for axis in 0..2 {
            position[axis] = ((point[axis] - self.target[axis]) / self.target[axis + 2]
                * sizes[axis] as f32
                - 0.5)
                .clamp(0.0, (sizes[axis] - 1) as f32);
        }
        let [left, top] = position.map(|value| value.floor() as usize);
        let right = (left + 1).min(extent.width - 1);
        let bottom = (top + 1).min(extent.height - 1);
        let pixels = self.raster.pixels();
        let values = [
            pixels[top * extent.width + left],
            pixels[top * extent.width + right],
            pixels[bottom * extent.width + left],
            pixels[bottom * extent.width + right],
        ];
        let fraction = position.map(f32::fract);
        let coverage = self.coverage([x, y]);
        let mut result = 0;
        for shift in [0, 8, 16, 24] {
            let channels = values.map(|pixel| ((pixel >> shift) & 255) as f32);
            let top = channels[0] + (channels[1] - channels[0]) * fraction[0];
            let bottom = channels[2] + (channels[3] - channels[2]) * fraction[0];
            result |= (((top + (bottom - top) * fraction[1]) * coverage).round() as u32) << shift;
        }
        result
    }
    fn coverage(&self, pixel: [usize; 2]) -> f32 {
        let mut count = 0;
        for row in 0..4 {
            for column in 0..4 {
                count += usize::from(self.clip.contains([
                    pixel[0] as f32 + (column as f32 + 0.5) / 4.0,
                    pixel[1] as f32 + (row as f32 + 0.5) / 4.0,
                ]));
            }
        }
        count as f32 / 16.0
    }
}
