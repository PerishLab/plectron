use domengine::{Available, Constraints, Content, Input, Layout, Sheet, html};
use fixtures::Text;

#[test]
fn specimens() {
    let specimens = spec::typography::run().unwrap();
    let app = fixtures::Application::new("clusters");
    let markup = app.load("/index.html").unwrap();
    let styles = app.load("/style.css").unwrap();
    let document = html::parse(std::str::from_utf8(&markup.body).unwrap()).unwrap();
    let sheet = Sheet::parse(std::str::from_utf8(&styles.body).unwrap()).unwrap();
    let layout = Layout::build(
        &Input::capture(&document, &sheet).unwrap(),
        Constraints {
            available: [Available::Definite(720), Available::Max],
            known: [None; 2],
        },
        &Text,
    )
    .unwrap();
    assert_eq!(specimens.len(), 10);
    for specimen in &specimens {
        let element = document.find(&specimen.id).unwrap();
        let node = document.entry(element).unwrap().children[0];
        let text = layout
            .fragments()
            .iter()
            .find(|fragment| {
                fragment.node == Some(node.id()) && matches!(fragment.content, Content::Text { .. })
            })
            .unwrap();
        let Content::Text {
            font,
            glyphs,
            clusters,
            ..
        } = &text.content
        else {
            unreachable!();
        };
        assert_eq!(*font, specimen.run.spans[0].font);
        assert_eq!(
            font.face,
            fixtures::face("lato", textengine::Weight::Normal)
        );
        assert_eq!(font.size, 28);
        let bidi =
            textengine::bidi::Bidi::resolve(&specimen.source, textengine::bidi::Direction::Ltr)
                .unwrap();
        let mut run = specimen.run.clone();
        run.orient(&specimen.source, &bidi).unwrap();
        let mut order: Vec<_> = bidi
            .visual(0..specimen.source.len())
            .iter()
            .map(|index| {
                run.clusters
                    .iter()
                    .position(|cluster| cluster.source.contains(&bidi.units[*index].source.start))
                    .unwrap()
            })
            .collect();
        order.dedup();
        let expected: Vec<_> = order
            .iter()
            .flat_map(|index| run.clusters[*index].glyphs.clone())
            .collect();
        assert_eq!(
            glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            expected
                .iter()
                .map(|index| run.glyphs[*index].id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            clusters.to_vec(),
            order
                .iter()
                .flat_map(|index| {
                    let cluster = &run.clusters[*index];
                    cluster.glyphs.clone().map(|_| cluster.source.clone())
                })
                .collect::<Vec<_>>()
        );
        let restored: String = specimen
            .characters
            .iter()
            .map(|character| &specimen.source[character.source.clone()])
            .collect();
        assert_eq!(restored, specimen.source);
    }
    let source = |id| {
        &specimens
            .iter()
            .find(|sample| sample.id == id)
            .unwrap()
            .source
    };
    assert_eq!(source("combining"), "é / e\u{0301}   Å / A\u{030a}");
    assert_eq!(source("ligatures"), "fi ffi / \u{fb01} \u{fb03}");
    assert_eq!(source("indic"), "\u{0915}\u{094d}\u{0937}\u{093f}");
    assert_eq!(
        source("emoji"),
        "\u{1f469}\u{1f3fd}\u{200d}\u{1f4bb} \u{1f1e8}\u{1f1f3} 1\u{fe0f}\u{20e3}"
    );
}
