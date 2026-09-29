use super::{Binding, Chord, Set, command};
use crate::Edit;

pub fn editor() -> Set<Edit> {
    let entries = [
        ("Backspace", "backward", true),
        ("Delete", "forward", true),
        ("ArrowLeft", "left", true),
        ("ArrowRight", "right", true),
        ("Home", "start", true),
        ("End", "end", true),
        ("Shift+ArrowLeft", "extend:left", true),
        ("Shift+ArrowRight", "extend:right", true),
        ("Shift+Home", "extend:start", true),
        ("Shift+End", "extend:end", true),
        ("Primary+KeyA", "selectAll", true),
        ("Enter", "commit", false),
        ("Escape", "blur", false),
    ];
    set(&entries)
}
pub fn page() -> Set<Edit> {
    set(&[
        ("Tab", "next", false),
        ("Enter", "activate", false),
        ("Space", "activate", false),
    ])
}
fn set(entries: &[(&str, &str, bool)]) -> Set<Edit> {
    Set {
        priority: -100,
        bindings: entries
            .iter()
            .map(|(chord, name, repeat)| Binding {
                chord: Chord::parse(chord).expect("valid default chord"),
                command: command(name).expect("valid default command"),
                repeat: *repeat,
                consume: true,
            })
            .collect(),
    }
}
