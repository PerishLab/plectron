use crate::Fault;

pub(crate) struct Parse;

impl Parse {
    pub(crate) fn offset(value: &str) -> Result<f64, Fault> {
        match value {
            "from" => Ok(0.0),
            "to" => Ok(1.0),
            _ => value
                .strip_suffix('%')
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
                .map(|value| value / 100.0)
                .ok_or_else(|| Fault("invalid keyframe offset".into())),
        }
    }

    pub(crate) fn valid(value: &str) -> Result<(), Fault> {
        if value.is_empty()
            || value == "none"
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            Err(Fault("invalid animation name".into()))
        } else {
            Ok(())
        }
    }

    pub(crate) fn name(value: &str) -> Result<Option<String>, Fault> {
        if value == "none" {
            return Ok(None);
        }
        Self::valid(value)?;
        Ok(Some(value.into()))
    }

    fn time(value: &str) -> Result<f64, Fault> {
        let (number, multiplier) = value
            .strip_suffix("ms")
            .map(|value| (value, 1000.0))
            .or_else(|| value.strip_suffix('s').map(|value| (value, 1_000_000.0)))
            .ok_or_else(|| Fault("animation time requires ms or s".into()))?;
        let value = number
            .parse::<f64>()
            .map_err(|_| Fault("invalid animation time".into()))?;
        let micros = value * multiplier;
        if !micros.is_finite() || micros.abs() > 3_600_000_000.0 {
            return Err(Fault("animation time exceeds budget".into()));
        }
        Ok(micros)
    }

    pub(crate) fn duration(value: &str) -> Result<u64, Fault> {
        let value = Self::time(value)?;
        if value < 0.0 {
            return Err(Fault("negative animation duration".into()));
        }
        Ok(value.round() as u64)
    }

    pub(crate) fn delay(value: &str) -> Result<i64, Fault> {
        Ok(Self::time(value)?.round() as i64)
    }

    pub(crate) fn iterations(value: &str) -> Result<Option<f64>, Fault> {
        if value == "infinite" {
            return Ok(None);
        }
        value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && (0.0..=4096.0).contains(value))
            .map(Some)
            .ok_or_else(|| Fault("invalid animation iteration count".into()))
    }

    pub(crate) fn playing(value: &str) -> Result<bool, Fault> {
        match value {
            "running" => Ok(true),
            "paused" => Ok(false),
            _ => Err(Fault("invalid animation play state".into())),
        }
    }
}
