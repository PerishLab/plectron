use shared::Resource;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    L,
    R,
    Al,
    En,
    Es,
    Et,
    An,
    Cs,
    Nsm,
    Bn,
    B,
    S,
    Ws,
    On,
    Lri,
    Rli,
    Fsi,
    Pdi,
    Legacy,
}
impl Class {
    pub(super) fn strong(self) -> Option<Self> {
        match self {
            Self::L => Some(Self::L),
            Self::R | Self::Al | Self::En | Self::An => Some(Self::R),
            _ => None,
        }
    }
    pub(super) fn isolate(self) -> bool {
        matches!(self, Self::Lri | Self::Rli | Self::Fsi)
    }
}
struct Tables {
    classes: Vec<Class>,
    brackets: Vec<(char, char, bool)>,
    mirrors: Vec<(char, char)>,
}
fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut classes = vec![Class::L; 0x110000];
        let data = Resource::new(include_bytes!(
            "../../resources/unicode/DerivedBidiClass.txt"
        ))
        .text()
        .expect("Unicode bidi data");
        for line in data
            .lines()
            .filter_map(|line| line.strip_prefix("# @missing: "))
        {
            assign(&mut classes, line);
        }
        for line in data
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        {
            assign(&mut classes, line);
        }
        let brackets = records(Resource::new(include_bytes!(
            "../../resources/unicode/BidiBrackets.txt"
        )))
        .map(|parts| (character(parts[0]), character(parts[1]), parts[2] == "o"))
        .collect();
        let mirrors = records(Resource::new(include_bytes!(
            "../../resources/unicode/BidiMirroring.txt"
        )))
        .map(|parts| (character(parts[0]), character(parts[1])))
        .collect();
        Tables {
            classes,
            brackets,
            mirrors,
        }
    })
}
pub fn class(character: char) -> Class {
    tables().classes[character as usize]
}
pub fn mirror(character: char) -> char {
    let entries = &tables().mirrors;
    entries
        .binary_search_by_key(&character, |entry| entry.0)
        .ok()
        .map_or(character, |index| entries[index].1)
}
pub(super) fn bracket(character: char) -> Option<(char, bool)> {
    let entries = &tables().brackets;
    entries
        .binary_search_by_key(&character, |entry| entry.0)
        .ok()
        .map(|index| (entries[index].1, entries[index].2))
}
fn records(resource: Resource) -> impl Iterator<Item = Vec<&'static str>> {
    resource
        .text()
        .expect("Unicode data")
        .lines()
        .map(|line| line.split('#').next().unwrap())
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split(';').map(str::trim).collect())
}
fn character(value: &str) -> char {
    char::from_u32(u32::from_str_radix(value, 16).expect("Unicode codepoint"))
        .expect("Unicode scalar")
}
fn assign(classes: &mut [Class], line: &str) {
    let parts: Vec<_> = line
        .split('#')
        .next()
        .unwrap()
        .split(';')
        .map(str::trim)
        .collect();
    let range: Vec<_> = parts[0]
        .split("..")
        .map(|value| usize::from_str_radix(value, 16).expect("Unicode range"))
        .collect();
    classes[range[0]..=*range.last().unwrap()].fill(parse(parts[1]));
}
fn parse(name: &str) -> Class {
    use Class::*;
    match name {
        "L" | "Left_To_Right" => L,
        "R" | "Right_To_Left" => R,
        "AL" | "Arabic_Letter" => Al,
        "EN" => En,
        "ES" => Es,
        "ET" | "European_Terminator" => Et,
        "AN" => An,
        "CS" => Cs,
        "NSM" => Nsm,
        "BN" => Bn,
        "B" => B,
        "S" => S,
        "WS" => Ws,
        "ON" => On,
        "LRI" => Lri,
        "RLI" => Rli,
        "FSI" => Fsi,
        "PDI" => Pdi,
        "LRE" | "RLE" | "LRO" | "RLO" | "PDF" => Legacy,
        _ => panic!("unknown Unicode bidi class: {name}"),
    }
}
