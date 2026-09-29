use super::data::Class;

pub(super) fn resolve(types: &mut [Class], boundary: Class) {
    use Class::*;
    let mut previous = boundary;
    for class in types.iter_mut() {
        if *class == Nsm {
            *class = if previous.isolate() || previous == Pdi {
                On
            } else {
                previous
            };
        }
        previous = *class;
    }
    let mut strong = boundary;
    for class in types.iter_mut() {
        match *class {
            R | L | Al => strong = *class,
            En if strong == Al => *class = An,
            _ => (),
        }
    }
    for class in types.iter_mut() {
        if *class == Al {
            *class = R;
        }
    }
    for index in 1..types.len().saturating_sub(1) {
        let left = types[index - 1];
        let right = types[index + 1];
        if let Some(class) = separator(types[index], left, right) {
            types[index] = class;
        }
    }
    let mut start = 0;
    while start < types.len() {
        if types[start] != Et {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < types.len() && types[end] == Et {
            end += 1;
        }
        if start.checked_sub(1).is_some_and(|index| types[index] == En)
            || types.get(end) == Some(&En)
        {
            types[start..end].fill(En);
        }
        start = end;
    }
    for class in types.iter_mut() {
        if matches!(*class, Es | Et | Cs) {
            *class = On;
        }
    }
    let mut strong = boundary;
    for class in types.iter_mut() {
        match *class {
            R | L => strong = *class,
            En if strong == L => *class = L,
            _ => (),
        }
    }
}

fn separator(class: Class, left: Class, right: Class) -> Option<Class> {
    use Class::*;
    match (class, left, right) {
        (Es, En, En) | (Cs, En, En) => Some(En),
        (Cs, An, An) => Some(An),
        _ => None,
    }
}
