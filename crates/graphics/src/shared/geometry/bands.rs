use crate::shared::Rect;
use std::ops::Range;

#[derive(Debug, Default)]
pub struct Bands {
    origin: usize,
    height: usize,
    stride: usize,
    count: usize,
    data: Vec<u32>,
}
impl Bands {
    pub(super) fn new(edges: &[[f32; 4]], bounds: Rect) -> Self {
        let mut result = Self {
            origin: bounds.y,
            height: bounds.height,
            stride: bounds.height.div_ceil(64).max(8),
            ..Self::default()
        };
        let budget = edges.len().saturating_mul(8).max(256);
        while result.references(edges) > budget {
            result.stride = result.stride.saturating_mul(2);
        }
        result.count = result.height.div_ceil(result.stride);
        let mut counts = vec![0; result.count];
        for &edge in edges {
            for band in result.span(edge) {
                counts[band] += 1;
            }
        }
        result.data.push((result.count + 1) as u32);
        for count in counts {
            result.data.push(result.data.last().unwrap() + count);
        }
        let mut cursors = result.data[..result.count].to_vec();
        result.data.resize(*result.data.last().unwrap() as usize, 0);
        for (index, &edge) in edges.iter().enumerate() {
            for band in result.span(edge) {
                result.data[cursors[band] as usize] = index as u32;
                cursors[band] += 1;
            }
        }
        result
    }
    fn span(&self, edge: [f32; 4]) -> Range<usize> {
        let low = edge[1].min(edge[3]);
        let high = edge[1].max(edge[3]);
        if low == high {
            return 0..0;
        }
        let top = (low.floor().max(0.0) as usize)
            .saturating_sub(self.origin)
            .min(self.height);
        let bottom = (high.ceil().max(0.0) as usize)
            .saturating_sub(self.origin)
            .min(self.height);
        if top >= bottom {
            return 0..0;
        }
        top / self.stride..bottom.div_ceil(self.stride)
    }
    fn references(&self, edges: &[[f32; 4]]) -> usize {
        edges.iter().map(|&edge| self.span(edge).len()).sum()
    }
    pub fn origin(&self) -> usize {
        self.origin
    }
    pub fn stride(&self) -> usize {
        self.stride
    }
    pub(super) fn retained(&self) -> usize {
        self.data.capacity() * 4
    }
    pub fn data(&self) -> &[u32] {
        &self.data
    }
    pub fn candidates(&self, y: usize) -> &[u32] {
        let Some(row) = y.checked_sub(self.origin).filter(|row| *row < self.height) else {
            return &[];
        };
        let band = row / self.stride;
        &self.data[self.data[band] as usize..self.data[band + 1] as usize]
    }
    pub(super) fn visits(&self) -> u64 {
        if self.origin.saturating_add(self.height) >= 2_097_152 {
            let maximum = self.data[..=self.count]
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .max()
                .unwrap_or(0);
            return (self.height as u64).saturating_mul(u64::from(maximum));
        }
        (0..self.count)
            .map(|band| {
                let rows = (self.height - band * self.stride).min(self.stride);
                rows as u64 * u64::from(self.data[band + 1] - self.data[band])
            })
            .sum()
    }
}
