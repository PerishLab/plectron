#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Property {
    #[default]
    Other,
    Cr,
    Lf,
    Control,
    Extend,
    Zwj,
    Spacing,
    Prepend,
    Regional,
    L,
    V,
    T,
    Lv,
    Lvt,
    Consonant,
    Linker,
    Picture,
}
pub(super) struct Tables;
type Attributes = (Property, Property, bool);
include!(concat!(env!("OUT_DIR"), "/grapheme.rs"));
