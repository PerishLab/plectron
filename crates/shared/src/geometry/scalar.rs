#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scalar(i64);

impl Scalar {
    pub const ZERO: Self = Self(0);
    pub fn new(value: f64) -> Self {
        Self((value * 65536.0).round() as i64)
    }
    pub fn value(self) -> f64 {
        self.0 as f64 / 65536.0
    }
    pub fn ceil(self) -> usize {
        self.0
            .div_euclid(65536)
            .saturating_add(i64::from(self.0.rem_euclid(65536) != 0))
            .max(0) as usize
    }
    pub fn floor(self) -> i64 {
        self.0.div_euclid(65536)
    }
    pub fn trim(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0).max(0))
    }
    pub fn subtract(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}
impl From<i64> for Scalar {
    fn from(value: i64) -> Self {
        Self(value.saturating_mul(65536))
    }
}
impl Add for Scalar {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}
impl Sub for Scalar {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}
impl Mul<usize> for Scalar {
    type Output = Self;
    fn mul(self, count: usize) -> Self {
        Self(self.0.saturating_mul(count as i64))
    }
}
impl Div<usize> for Scalar {
    type Output = Self;
    fn div(self, count: usize) -> Self {
        Self(self.0 / count as i64)
    }
}
impl PartialEq<i64> for Scalar {
    fn eq(&self, value: &i64) -> bool {
        *self == Self::from(*value)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Scalar {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.value())
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(deserializer)?;
        if !value.is_finite() || value.abs() > 140_737_488_355_327.0 {
            return Err(serde::de::Error::custom("invalid fixed-point coordinate"));
        }
        Ok(Self::new(value))
    }
}
