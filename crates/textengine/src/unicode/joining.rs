use shared::Resource;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Joining {
    Unjoined,
    Right,
    Left,
    Dual,
    Causing,
    Transparent,
}
pub(crate) fn joining(character: char) -> Joining {
    static TYPES: OnceLock<Vec<Joining>> = OnceLock::new();
    TYPES.get_or_init(|| {
        let mut types = vec![Joining::Unjoined; 0x110000];
        for line in Resource::new(include_bytes!(
            "../../resources/unicode/DerivedJoiningType.txt"
        ))
        .text()
        .expect("Unicode joining data")
        .lines()
        {
            let parts: Vec<_> = line
                .split('#')
                .next()
                .unwrap()
                .split(';')
                .map(str::trim)
                .collect();
            if parts.len() < 2 {
                continue;
            }
            let range: Vec<_> = parts[0]
                .split("..")
                .map(|part| usize::from_str_radix(part, 16).expect("joining range"))
                .collect();
            let value = match parts[1] {
                "R" => Joining::Right,
                "L" => Joining::Left,
                "D" => Joining::Dual,
                "C" => Joining::Causing,
                "T" => Joining::Transparent,
                _ => Joining::Unjoined,
            };
            types[range[0]..=*range.last().unwrap()].fill(value);
        }
        types
    })[character as usize]
}
pub(crate) fn forms(source: &str) -> Vec<Option<[u8; 4]>> {
    use Joining::*;
    let types: Vec<_> = source.chars().map(joining).collect();
    let mut forms = vec![None; types.len()];
    let mut previous = Option::<usize>::None;
    for (index, current) in types.iter().enumerate() {
        if *current == Transparent {
            continue;
        }
        if matches!(current, Dual | Right | Left) {
            forms[index] = Some(*b"isol");
        }
        if let Some(previous) = previous
            && types[previous].connects(*current)
        {
            forms[previous] = initial(forms[previous]);
            forms[index] = forms[index].map(|_| *b"fina");
        }
        previous = Some(index);
    }
    forms
}
pub fn hidden(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200b}'
            | '\u{2060}'
            | '\u{200c}'
            | '\u{200d}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2066}'..='\u{2069}'
    )
}

pub fn connections(source: &str) -> std::collections::BTreeSet<usize> {
    let units: Vec<_> = source
        .char_indices()
        .map(|(offset, character)| (offset, joining(character)))
        .collect();
    let mut boundaries = std::collections::BTreeSet::new();
    let mut previous = None;
    for (index, (_, current)) in units.iter().enumerate() {
        if *current == Joining::Transparent {
            continue;
        }
        if let Some(previous) = previous {
            let (_, before): (usize, Joining) = units[previous];
            if before.connects(*current) {
                boundaries.extend(
                    units[previous + 1..=index]
                        .iter()
                        .map(|(offset, _)| *offset),
                );
            }
        }
        previous = Some(index);
    }
    boundaries
}
impl Joining {
    fn connects(self, next: Self) -> bool {
        matches!(self, Self::Dual | Self::Left | Self::Causing)
            && matches!(next, Self::Dual | Self::Right | Self::Causing)
    }
}

fn initial(form: Option<[u8; 4]>) -> Option<[u8; 4]> {
    form.map(|form| if form == *b"fina" { *b"medi" } else { *b"init" })
}
