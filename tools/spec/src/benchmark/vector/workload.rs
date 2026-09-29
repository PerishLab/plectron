use graphics::shared::{Extent, Font, Frame, Glyph, Item, Scene, Visual, Weight};

pub(in crate::benchmark) fn frame(scenario: &str) -> Result<Frame, graphics::shared::Fault> {
    let (family, text, size, columns, rows) = match scenario {
        "dense" => ("noto-sc", "现代浏览器曲线几何渲染", 28, 12, 8),
        "large" => ("lato", "OgQ@", 160, 4, 1),
        "repeat" => ("lato", "g", 32, 16, 8),
        "scale" => ("lato", "OgQ@", 48, 8, 4),
        _ => return Err(graphics::shared::Fault("unknown vector workload".into())),
    };
    let font = Font {
        face: fixtures::face(family, Weight::Normal),
        size: size.into(),
        weight: Weight::Normal,
    };
    let text: Vec<_> = text.chars().collect();
    let glyphs = (0..columns * rows)
        .map(|index| Glyph {
            id: font.index(text[index % text.len()]),
            x: ((index % columns * size as usize + 4) as i64).into(),
            y: ((index / columns * (size as usize + 8) + 4) as i64).into(),
        })
        .collect();
    let extent = Extent {
        width: columns * size as usize + 8,
        height: rows * (size as usize + 8) + size as usize / 2,
    };
    Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Text {
                font,
                clip: None,
                glyphs,
                color: 0x224466,
            }],
        )?,
        Visual::new(extent),
    )
}
