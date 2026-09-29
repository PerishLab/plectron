use crate::shared::Rect;

pub fn encode(extent: Rect, rectangles: &[(Rect, u32)]) -> Vec<f32> {
    let mut output = Vec::new();
    for (rect, color) in rectangles {
        let left = (rect.x as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0;
        let right =
            ((rect.x + rect.width) as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (rect.y as f32 - extent.y as f32) / extent.height as f32 * 2.0;
        let bottom =
            1.0 - ((rect.y + rect.height) as f32 - extent.y as f32) / extent.height as f32 * 2.0;
        let channels = [
            (color >> 16) as f32 / 255.0,
            ((color >> 8) & 255) as f32 / 255.0,
            (color & 255) as f32 / 255.0,
        ];
        for point in [
            [left, top],
            [left, bottom],
            [right, top],
            [right, top],
            [left, bottom],
            [right, bottom],
        ] {
            output.extend_from_slice(&[point[0], point[1], channels[0], channels[1], channels[2]]);
        }
    }
    output
}

pub(crate) fn placements(extent: Rect, entries: &[crate::metal::buffer::Entry<'_>]) -> Vec<f32> {
    let mut output = Vec::with_capacity(entries.len() * 8);
    for (coverage, color) in entries {
        let rect = coverage.bounds;
        output.extend_from_slice(&[
            (rect.x as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0,
            1.0 - (rect.y as f32 - extent.y as f32) / extent.height as f32 * 2.0,
            ((rect.x + rect.width) as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0,
            1.0 - ((rect.y + rect.height) as f32 - extent.y as f32) / extent.height as f32 * 2.0,
            (color >> 16) as f32 / 255.0,
            ((color >> 8) & 255) as f32 / 255.0,
            (color & 255) as f32 / 255.0,
            0.0,
        ]);
    }
    output
}
