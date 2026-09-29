use super::{Class, context::Context};

impl Context<'_> {
    pub(super) fn hyphen(&self) -> bool {
        use Class::*;
        let a = self.units[self.index - 1].property.class;
        let b = self.units[self.index].property.class;
        if !matches!(a, Hy | Hh) {
            return false;
        }
        if b.alphabetic()
            && self.before(2).is_none_or(|unit| {
                unit.property.class.hard() || matches!(unit.property.class, Sp | Zw | Cb | Gl)
            })
        {
            return true;
        }
        b != Hl && self.before(2).is_some_and(|unit| unit.property.class == Hl)
    }
    pub(super) fn numeric(&self) -> bool {
        use Class::*;
        let left = &self.units[self.index - 1];
        let a = left.property.class;
        let b = self.units[self.index].property.class;
        let suffix =
            left.number || (matches!(a, Cl | Cp) && self.before(2).is_some_and(|unit| unit.number));
        if matches!(b, Po | Pr) && suffix {
            return true;
        }
        if matches!(a, Po | Pr) {
            if b == Nu {
                return true;
            }
            if b == Op
                && self.after(1).is_some_and(|unit| {
                    unit.property.class == Nu
                        || (unit.property.class == Is
                            && self.after(2).is_some_and(|unit| unit.property.class == Nu))
                })
            {
                return true;
            }
        }
        b == Nu && (matches!(a, Hy | Is) || left.number)
    }
    pub(super) fn syllable(&self) -> bool {
        use Class::*;
        let left = &self.units[self.index - 1];
        let right = &self.units[self.index];
        let (a, b) = (left.property.class, right.property.class);
        let hangul = match a {
            Jl => matches!(b, Jl | Jv | H2 | H3),
            Jv | H2 => matches!(b, Jv | Jt),
            Jt | H3 => b == Jt,
            _ => false,
        };
        if hangul {
            return true;
        }
        let initial = matches!(a, Ak | As) || left.dotted;
        let finality = matches!(b, Ak | As) || right.dotted;
        if a == Ap && finality {
            return true;
        }
        if initial && matches!(b, Vf | Vi) {
            return true;
        }
        let consonant = b == Ak || right.dotted;
        if a == Vi
            && consonant
            && self
                .before(2)
                .is_some_and(|unit| matches!(unit.property.class, Ak | As) || unit.dotted)
        {
            return true;
        }
        initial && finality && self.after(1).is_some_and(|unit| unit.property.class == Vf)
    }
}
