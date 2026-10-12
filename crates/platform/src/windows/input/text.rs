use super::Error;

#[derive(Default)]
pub(super) struct Decoder(Option<u16>);
impl Decoder {
    pub fn clear(&mut self) {
        self.0 = None;
    }
    pub fn unit(&mut self, value: usize) -> Result<Option<String>, Error> {
        let unit = u16::try_from(value).map_err(|_| Error("invalid native UTF-16 unit".into()))?;
        if let Some(high) = self.0.take() {
            let text = String::from_utf16(&[high, unit])
                .map_err(|_| Error("unpaired native UTF-16 surrogate".into()))?;
            return Ok(Some(text));
        }
        if (0xd800..=0xdbff).contains(&unit) {
            self.0 = Some(unit);
            return Ok(None);
        }
        let text = String::from_utf16(&[unit])
            .map_err(|_| Error("unpaired native UTF-16 surrogate".into()))?;
        if text.chars().any(char::is_control) {
            return Ok(None);
        }
        Ok(Some(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar() {
        let mut decoder = Decoder::default();
        assert_eq!(decoder.unit(0x4e2d).unwrap().as_deref(), Some("中"));
        assert_eq!(decoder.unit(0xd83d).unwrap(), None);
        assert_eq!(decoder.unit(0xde42).unwrap().as_deref(), Some("🙂"));
        assert!(decoder.unit(0xdc00).is_err());
        decoder.unit(0xd800).unwrap();
        assert!(decoder.unit(65).is_err());
        assert!(decoder.unit(0x10000).is_err());
        decoder.unit(0xd800).unwrap();
        decoder.clear();
        assert_eq!(decoder.unit(65).unwrap().as_deref(), Some("A"));
        assert_eq!(decoder.unit(13).unwrap(), None);
    }
}
