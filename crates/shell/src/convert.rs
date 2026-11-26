use platform::shared::{Composition, Focus, Point};

pub(crate) fn coordinates(point: Point) -> Option<(usize, usize)> {
    (point.x >= 0 && point.y >= 0).then_some((point.x as usize, point.y as usize))
}

pub(crate) fn compose(input: Composition) -> webengine::ime::Input {
    match input {
        Composition::Mark {
            id,
            text,
            selection,
        } => webengine::ime::Input::Mark {
            id: webengine::ime::Id(id.0),
            text,
            selection: domengine::Selection {
                anchor: selection.anchor,
                focus: selection.focus,
            },
        },
        Composition::Commit { id, text } => webengine::ime::Input::Commit {
            id: webengine::ime::Id(id.0),
            text,
        },
        Composition::Cancel { id } => webengine::ime::Input::Cancel {
            id: webengine::ime::Id(id.0),
        },
    }
}

pub(crate) fn focus(context: webengine::Focus) -> Focus {
    Focus {
        target: context.target,
        text: context.text,
        selection: platform::shared::Selection {
            anchor: context.selection.anchor,
            focus: context.selection.focus,
        },
        caret: platform::shared::Caret {
            x: context.caret.x,
            y: context.caret.y,
            width: context.caret.width,
            height: context.caret.height,
        },
    }
}
