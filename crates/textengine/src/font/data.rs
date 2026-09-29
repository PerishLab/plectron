use super::Problem;

#[derive(Clone, Copy)]
pub(crate) struct Data<'a>(pub &'a [u8]);
impl<'a> Data<'a> {
    pub fn range(self, source: &[u8]) -> std::ops::Range<usize> {
        if self.0.is_empty() {
            return 0..0;
        }
        let start = self.0.as_ptr().addr() - source.as_ptr().addr();
        let range = start..start + self.0.len();
        assert!(range.end <= source.len());
        range
    }
    pub fn slice(self, start: usize, size: usize) -> Result<Self, Problem> {
        self.0
            .get(start..start.checked_add(size).ok_or(Problem::Malformed)?)
            .map(Self)
            .ok_or(Problem::Malformed)
    }
    pub fn tail(self, start: usize) -> Result<Self, Problem> {
        Ok(Self(self.0.get(start..).ok_or(Problem::Malformed)?))
    }
    pub fn offset(self, index: usize) -> Result<Self, Problem> {
        let offset = usize::from(self.word(index)?);
        if offset == 0 {
            return Err(Problem::Malformed);
        }
        self.tail(offset)
    }
    pub fn byte(self, index: usize) -> Result<u8, Problem> {
        self.0.get(index).copied().ok_or(Problem::Malformed)
    }
    pub fn word(self, index: usize) -> Result<u16, Problem> {
        Ok(u16::from_be_bytes(
            self.slice(index, 2)?
                .0
                .try_into()
                .map_err(|_| Problem::Malformed)?,
        ))
    }
    pub fn signed(self, index: usize) -> Result<i16, Problem> {
        Ok(self.word(index)? as i16)
    }
    pub fn long(self, index: usize) -> Result<u32, Problem> {
        Ok(u32::from_be_bytes(
            self.slice(index, 4)?
                .0
                .try_into()
                .map_err(|_| Problem::Malformed)?,
        ))
    }
}
pub(crate) struct Cursor<'a> {
    pub data: Data<'a>,
    pub offset: usize,
}
impl Cursor<'_> {
    pub fn byte(&mut self) -> Result<u8, Problem> {
        let value = self.data.byte(self.offset)?;
        self.offset += 1;
        Ok(value)
    }
    pub fn word(&mut self) -> Result<u16, Problem> {
        let value = self.data.word(self.offset)?;
        self.offset += 2;
        Ok(value)
    }
    pub fn signed(&mut self) -> Result<i16, Problem> {
        Ok(self.word()? as i16)
    }
    pub fn skip(&mut self, count: usize) -> Result<(), Problem> {
        self.data.slice(self.offset, count)?;
        self.offset += count;
        Ok(())
    }
    pub fn factor(&mut self) -> Result<f64, Problem> {
        Ok(f64::from(self.signed()?) / 16384.0)
    }
}
