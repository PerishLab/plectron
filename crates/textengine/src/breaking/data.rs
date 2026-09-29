#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Class {
    #[default]
    Al,
    Ai,
    Ak,
    Ap,
    As,
    B2,
    Ba,
    Bb,
    Bk,
    Cb,
    Cj,
    Cl,
    Cm,
    Cp,
    Cr,
    Eb,
    Em,
    Ex,
    Gl,
    H2,
    H3,
    Hh,
    Hl,
    Hy,
    Id,
    In,
    Is,
    Jl,
    Jt,
    Jv,
    Lf,
    Nl,
    Ns,
    Nu,
    Op,
    Po,
    Pr,
    Qu,
    Ri,
    Sa,
    Sg,
    Sp,
    Sy,
    Vf,
    Vi,
    Wj,
    Xx,
    Zw,
    Zwj,
}
#[derive(Clone, Copy, Default)]
pub(super) struct Property {
    pub class: Class,
    pub east: bool,
    pub initial: bool,
    pub finality: bool,
    pub mark: bool,
    pub letter: bool,
    pub separator: bool,
    pub unassigned: bool,
}
pub(super) fn property(character: char) -> Property {
    generated(character)
}
include!(concat!(env!("OUT_DIR"), "/breaking.rs"));
impl Class {
    pub fn word(self) -> bool {
        self.alphabetic() || self == Self::Nu
    }
    pub fn hard(self) -> bool {
        matches!(self, Self::Bk | Self::Cr | Self::Lf | Self::Nl)
    }
    pub fn alphabetic(self) -> bool {
        matches!(self, Self::Al | Self::Hl)
    }
    pub fn hangul(self) -> bool {
        matches!(self, Self::Jl | Self::Jv | Self::Jt | Self::H2 | Self::H3)
    }
}
