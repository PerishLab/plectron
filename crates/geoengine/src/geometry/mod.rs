use crate::Error;
mod block;
mod visual;
pub use block::{Block, Child, Flow};
pub use visual::{Coverage, Span};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unit(i32);

impl Unit {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }

    pub fn positive(value: usize) -> Result<Self, Error> {
        i32::try_from(value)
            .map(Self)
            .map_err(|_| Error("geometry coordinate overflow".into()))
    }

    pub fn offset(self, other: Self) -> Result<Self, Error> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| Error("geometry coordinate overflow".into()))
    }

    pub fn difference(self, other: Self) -> Result<Self, Error> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or_else(|| Error("geometry coordinate overflow".into()))
    }
}

impl PartialEq<i32> for Unit {
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: Unit,
    pub y: Unit,
}

impl Point {
    pub fn positive(value: [usize; 2]) -> Result<Self, Error> {
        Ok(Self {
            x: Unit::positive(value[0])?,
            y: Unit::positive(value[1])?,
        })
    }

    pub fn array(self) -> Option<[usize; 2]> {
        if self.x.0 < 0 || self.y.0 < 0 {
            return None;
        }
        Some([self.x.0 as usize, self.y.0 as usize])
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: Unit,
    pub height: Unit,
}

impl Size {
    pub fn positive(value: [usize; 2]) -> Result<Self, Error> {
        Ok(Self {
            width: Unit::positive(value[0])?,
            height: Unit::positive(value[1])?,
        })
    }

    pub fn array(self) -> Option<[usize; 2]> {
        if self.width.0 < 0 || self.height.0 < 0 {
            return None;
        }
        Some([self.width.0 as usize, self.height.0 as usize])
    }
}

impl std::ops::Index<usize> for Size {
    type Output = Unit;

    fn index(&self, axis: usize) -> &Self::Output {
        match axis {
            0 => &self.width,
            1 => &self.height,
            _ => panic!("geometry axis is outside its dimensions"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

pub struct Flex {
    base: f64,
    minimum: f64,
    maximum: f64,
    grow: f64,
    shrink: f64,
    inset: usize,
    target: f64,
    frozen: bool,
}

impl Flex {
    pub fn new(base: usize, bounds: [usize; 2], factors: [f64; 2], inset: usize) -> Self {
        Self {
            base: base as f64,
            minimum: bounds[0] as f64,
            maximum: bounds[1] as f64,
            grow: factors[0],
            shrink: factors[1],
            inset,
            target: 0.0,
            frozen: false,
        }
    }

    pub fn minimize(&mut self) {
        self.base = self.minimum;
    }

    pub fn bounded(&self) -> usize {
        self.clamp(self.base) as usize
    }

    fn clamp(&self, value: f64) -> f64 {
        value.max(self.minimum).min(self.maximum)
    }

    fn factor(&self, grow: bool) -> f64 {
        if grow { self.grow } else { self.shrink }
    }

    fn inflexible(&self, grow: bool) -> bool {
        self.factor(grow) == 0.0
            || if grow {
                self.base > self.target
            } else {
                self.base < self.target
            }
    }

    fn weight(&self, grow: bool) -> f64 {
        self.factor(grow) * if grow { 1.0 } else { self.base }
    }
}

pub fn flex(
    items: &mut [Flex],
    available: Option<usize>,
    gap: usize,
    limit: usize,
) -> Result<Vec<usize>, Error> {
    let inset =
        items.iter().map(|item| item.inset).sum::<usize>() + gap * items.len().saturating_sub(1);
    let hypothetical = items.iter().map(|item| item.clamp(item.base)).sum::<f64>() + inset as f64;
    let available = available.map_or(hypothetical, |value| value as f64);
    let grow = hypothetical < available;
    for item in items.iter_mut() {
        item.target = item.clamp(item.base);
        item.frozen = item.inflexible(grow);
    }
    let initial = free(items, available, inset);
    for _ in 0..=items.len() {
        if items.iter().all(|item| item.frozen) {
            return quantize(items, limit);
        }
        let factor = items
            .iter()
            .filter(|item| !item.frozen)
            .map(|item| item.factor(grow))
            .sum::<f64>();
        let mut remaining = free(items, available, inset);
        if factor < 1.0 && (initial * factor).abs() < remaining.abs() {
            remaining = initial * factor;
        }
        distribute(items, (grow, remaining));
    }
    Err(Error("flex distribution did not converge".into()))
}

fn free(items: &[Flex], available: f64, inset: usize) -> f64 {
    available
        - inset as f64
        - items
            .iter()
            .map(|item| if item.frozen { item.target } else { item.base })
            .sum::<f64>()
}

fn distribute(items: &mut [Flex], (grow, free): (bool, f64)) {
    let weight = items
        .iter()
        .filter(|item| !item.frozen)
        .map(|item| item.weight(grow))
        .sum::<f64>();
    let mut violation = 0.0;
    let mut adjustments = Vec::with_capacity(items.len());
    for item in items.iter_mut() {
        if item.frozen {
            adjustments.push(0.0);
            continue;
        }
        let target = item.base
            + if weight == 0.0 {
                0.0
            } else {
                free * item.weight(grow) / weight
            };
        item.target = item.clamp(target);
        adjustments.push(item.target - target);
        violation += item.target - target;
    }
    for (item, adjustment) in items.iter_mut().zip(adjustments) {
        if violation.abs() < 0.0000001 || violation * adjustment > 0.0 {
            item.frozen = true;
        }
    }
}

fn quantize(items: &[Flex], limit: usize) -> Result<Vec<usize>, Error> {
    let mut total = 0.0;
    let mut previous = 0;
    let mut sizes = Vec::with_capacity(items.len());
    for item in items {
        total += item.target;
        if !total.is_finite() || total > limit as f64 {
            return Err(Error("flex size budget exhausted".into()));
        }
        let edge = total.round() as usize;
        sizes.push(edge - previous);
        previous = edge;
    }
    Ok(sizes)
}
