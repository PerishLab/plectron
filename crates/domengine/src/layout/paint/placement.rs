use crate::Item;

pub(super) fn shift(mut item: Item, delta: [i64; 2]) -> Item {
    let Item::Group { space, .. } = &mut item else {
        unreachable!("independent layout branches produce a root group");
    };
    for (translation, delta) in space.translation.iter_mut().zip(delta) {
        *translation += delta;
    }
    item
}

pub(super) fn update(mut item: Item, transition: crate::layout::delta::Transition) -> Item {
    use crate::layout::delta::Transition;
    let Item::Group { space, opacity, .. } = &mut item else {
        unreachable!("independent layout branches produce a root group");
    };
    let delta = match transition {
        Transition::Translate(delta) => delta,
        Transition::Opacity(value) => {
            *opacity = value;
            [0; 2]
        }
        Transition::Composite {
            translation,
            opacity: value,
        } => {
            *opacity = value;
            translation
        }
    };
    for (translation, delta) in space.translation.iter_mut().zip(delta) {
        *translation += delta;
    }
    item
}
