use super::{
    Unit,
    data::{self, Class},
};

pub(super) fn resolve(units: &[Unit], indices: &[usize], types: &mut [Class], boundary: Class) {
    brackets(units, indices, types, boundary);
    let mut start = 0;
    let mut previous = boundary;
    while start < types.len() {
        if let Some(strong) = types[start].strong() {
            previous = strong;
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < types.len() && types[end].strong().is_none() {
            end += 1;
        }
        let next = types
            .get(end)
            .and_then(|class| class.strong())
            .unwrap_or(boundary);
        types[start..end].fill(if previous == next { previous } else { boundary });
        start = end;
    }
}
fn brackets(units: &[Unit], indices: &[usize], types: &mut [Class], boundary: Class) {
    let mut stack: Vec<(usize, char)> = Vec::new();
    let mut pairs = Vec::new();
    for (index, source) in indices.iter().enumerate() {
        if types[index] != Class::On {
            continue;
        }
        let character = equivalent(units[*source].character);
        let Some((other, open)) = data::bracket(character) else {
            continue;
        };
        if open {
            if stack.len() == 63 {
                break;
            }
            stack.push((index, equivalent(other)));
        } else if let Some(position) = stack
            .iter()
            .rposition(|(_, expected)| *expected == character)
        {
            pairs.push((stack[position].0, index));
            stack.truncate(position);
        }
    }
    pairs.sort_unstable();
    for (start, end) in pairs {
        let found: Vec<_> = types[start + 1..end]
            .iter()
            .filter_map(|class| class.strong())
            .collect();
        let resolved = if found.contains(&boundary) {
            Some(boundary)
        } else if !found.is_empty() {
            Some(
                types[..start]
                    .iter()
                    .rev()
                    .find_map(|class| class.strong())
                    .unwrap_or(boundary),
            )
        } else {
            None
        };
        if let Some(resolved) = resolved {
            types[start] = resolved;
            types[end] = resolved;
            propagate(types, (units, indices), start, resolved);
            propagate(types, (units, indices), end, resolved);
        }
    }
}
fn equivalent(character: char) -> char {
    match character {
        '\u{2329}' => '\u{3008}',
        '\u{232a}' => '\u{3009}',
        _ => character,
    }
}

fn propagate(types: &mut [Class], source: (&[Unit], &[usize]), endpoint: usize, resolved: Class) {
    for (index, class) in types.iter_mut().enumerate().skip(endpoint + 1) {
        if source.0[source.1[index]].class != Class::Nsm {
            break;
        }
        *class = resolved;
    }
}
