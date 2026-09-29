use super::{Class, Kind, Unit};

pub(super) struct Context<'a> {
    pub units: &'a [Unit],
    pub index: usize,
}
impl Context<'_> {
    pub fn before(&self, distance: usize) -> Option<&Unit> {
        self.index
            .checked_sub(distance)
            .and_then(|index| self.units.get(index))
    }
    pub fn after(&self, distance: usize) -> Option<&Unit> {
        self.units.get(self.index + distance)
    }
    pub fn boundary(&self) -> Option<Kind> {
        use Class::*;
        let left = &self.units[self.index - 1];
        let right = &self.units[self.index];
        let (a, b) = (left.property.class, right.property.class);
        if a == Bk || (matches!(a, Cr | Lf | Nl) && (a, b) != (Cr, Lf)) {
            return Some(Kind::Hard);
        }
        if b.hard() || matches!(b, Sp | Zw) {
            return None;
        }
        let spaced = self.before(1 + left.spaces);
        if spaced.is_some_and(|unit| unit.property.class == Zw) {
            return Some(Kind::Soft);
        }
        if left.joined || matches!(a, Wj | Gl) || b == Wj {
            return None;
        }
        if b == Gl && !matches!(a, Sp | Ba | Hy | Hh) {
            return None;
        }
        if let Some(allowed) = self.punctuation() {
            return allowed.then_some(Kind::Soft);
        }
        if a == Sp || a == Cb || b == Cb {
            return Some(Kind::Soft);
        }
        if self.hyphen() || matches!(b, Ba | Hh | Hy | Ns | In) || a == Bb {
            return None;
        }
        if (a, b) == (Sy, Hl) {
            return None;
        }
        if (a.alphabetic() && b == Nu) || (a == Nu && b.alphabetic()) {
            return None;
        }
        if (a == Pr && matches!(b, Id | Eb | Em)) || (matches!(a, Id | Eb | Em) && b == Po) {
            return None;
        }
        if (matches!(a, Pr | Po) && b.alphabetic()) || (a.alphabetic() && matches!(b, Pr | Po)) {
            return None;
        }
        if self.numeric() || self.syllable() {
            return None;
        }
        if (a.hangul() && b == Po) || (a == Pr && b.hangul()) {
            return None;
        }
        if a.alphabetic() && b.alphabetic() {
            return None;
        }
        if a == Is && b.alphabetic() {
            return None;
        }
        if a.word() && b == Op && !right.property.east {
            return None;
        }
        if a == Cp && !left.property.east && b.word() {
            return None;
        }
        if a == Ri && b == Ri && left.regional % 2 == 1 {
            return None;
        }
        if b == Em && (a == Eb || left.picture) {
            return None;
        }
        Some(Kind::Soft)
    }
    fn punctuation(&self) -> Option<bool> {
        use Class::*;
        let left = &self.units[self.index - 1];
        let right = &self.units[self.index];
        let (a, b) = (left.property.class, right.property.class);
        let spaced = self.before(1 + left.spaces);
        if matches!(b, Cl | Cp | Ex | Sy) || spaced.is_some_and(|unit| unit.property.class == Op) {
            return Some(false);
        }
        if spaced.is_some_and(|unit| unit.property.class == Qu && unit.property.initial)
            && self.before(2 + left.spaces).is_none_or(|unit| {
                unit.property.class.hard() || matches!(unit.property.class, Op | Qu | Gl | Sp | Zw)
            })
        {
            return Some(false);
        }
        if b == Qu
            && right.property.finality
            && self.after(1).is_none_or(|unit| {
                unit.property.class.hard()
                    || matches!(
                        unit.property.class,
                        Sp | Gl | Wj | Cl | Qu | Cp | Ex | Is | Sy | Zw
                    )
            })
        {
            return Some(false);
        }
        if a == Sp && b == Is && self.after(1).is_some_and(|unit| unit.property.class == Nu) {
            return Some(true);
        }
        if b == Is {
            return Some(false);
        }
        if b == Ns && spaced.is_some_and(|unit| matches!(unit.property.class, Cl | Cp)) {
            return Some(false);
        }
        if b == B2 && spaced.is_some_and(|unit| unit.property.class == B2) {
            return Some(false);
        }
        if a == Sp {
            return Some(true);
        }
        if (b == Qu && !right.property.initial) || (a == Qu && !left.property.finality) {
            return Some(false);
        }
        if b == Qu && (!left.property.east || self.after(1).is_none_or(|unit| !unit.property.east))
        {
            return Some(false);
        }
        if a == Qu
            && (!right.property.east || self.before(2).is_none_or(|unit| !unit.property.east))
        {
            return Some(false);
        }
        None
    }
}
