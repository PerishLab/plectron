use graphics::shared::{Extent, Frame, Item, Scalar, Scene, Space, Visual};

pub(super) fn frame(scenario: &str, step: usize) -> Result<Frame, graphics::shared::Fault> {
    let source = super::super::vector::workload::frame("dense")?;
    let mut items = source.scene().items().to_vec();
    let Item::Text {
        glyphs,
        font,
        color,
        ..
    } = &mut items[0]
    else {
        unreachable!()
    };
    for glyph in glyphs.iter_mut() {
        glyph.x = glyph.x + 30.into();
        glyph.y = glyph.y + 30.into();
    }
    match scenario {
        "phase" => {
            for glyph in glyphs.iter_mut() {
                glyph.x = glyph.x + Scalar::new(step as f64 / 8.0);
            }
        }
        "color" => *color = 0x224466 ^ (step as u32 * 17493),
        "edit" => {
            glyphs[0].id =
                font.index(['语', '言', '字', '形', '数', '据', '测', '试', '运', '行'][step])
        }
        _ => (),
    }
    let extent = Extent {
        width: source.extent().width + 80,
        height: source.extent().height + 80,
    };
    let space = Space {
        id: 1,
        translation: if scenario == "translate" {
            [step as i64 * 3, step as i64 * 2]
        } else {
            [0; 2]
        },
        clip: if scenario == "scroll" {
            Some(
                graphics::shared::Rect {
                    x: 16,
                    y: 40,
                    width: extent.width - 32,
                    height: extent.height - 80,
                }
                .into(),
            )
        } else {
            None
        },
        scroll: (scenario == "scroll").then_some(64),
    };
    let scene = Scene::new(
        extent,
        0,
        vec![Item::Group {
            space,
            opacity: 255,
            items: items.into(),
        }],
    )?;
    let mut visual = Visual::new(extent);
    if scenario == "scroll" {
        visual.offsets.insert(1, step * 4);
    }
    Frame::new(scene, visual)
}
