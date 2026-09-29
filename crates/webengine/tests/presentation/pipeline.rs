use domengine::{Constraints, Extent, Input, Layout, Painter, Pipeline, Sheet, html};
use fixtures::Text;
use graphics::shared::{
    Backend, Cpu, Output, Preparation, State, Visual,
    operation::{Completion, Mapping},
};
use pipeline::{Graph, Trace, step};

#[test]
fn publication() {
    let document =
        html::parse("<div style='background-color: #112233;'><p>Hello</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let constraints = Constraints::viewport(Extent {
        width: 128,
        height: 128,
    });
    let reference = Layout::build(&input, constraints, &Text).unwrap();
    let mut engine = Pipeline::new(Text);
    let mut painter = Painter::default();
    let mut validation = 0;
    let mut trace = Trace::default();
    let (layout, scene) = webengine::operation::publication(
        &mut engine,
        &mut painter,
        &mut validation,
        (None, graphics::shared::CHECKS),
    )
    .observe((&input, constraints), &mut trace)
    .unwrap();
    assert_eq!(layout.fragments(), reference.fragments());
    assert_eq!(
        trace.spans.iter().map(|span| span.name).collect::<Vec<_>>(),
        ["dom.layout", "dom.paint", "web.publication"]
    );
    assert_eq!(trace.topology.as_ref().unwrap().vertices()[2].incoming, [1]);
    let frame = step(Mapping)
        .run((scene, Visual::new(layout.extent())))
        .unwrap();
    let expected = Cpu.render(&frame).unwrap();
    let output = Output::new(layout.extent(), 1.0).unwrap();
    let mut preparation = Preparation::default();
    let mut backend = Cpu;
    let mut work = graphics::shared::operation::submission(&mut preparation, &mut backend)
        .observe((&frame, output), &mut trace)
        .unwrap();
    assert_eq!(work, expected);
    assert_eq!(
        trace.spans.iter().map(|span| span.name).collect::<Vec<_>>(),
        ["render.preparation", "render.submission"]
    );
    assert_eq!(trace.topology.as_ref().unwrap().vertices()[1].incoming, [0]);
    assert_eq!(
        step(Completion {
            backend: &mut backend
        })
        .observe(&mut work, &mut trace)
        .unwrap(),
        State::Complete
    );
    assert_eq!(trace.spans.len(), 1);
    assert_eq!(trace.spans[0].name, "render.completion");
    let invalid = Output::new(
        Extent {
            width: 1,
            height: 1,
        },
        1.0,
    )
    .unwrap();
    assert!(
        graphics::shared::operation::submission(&mut preparation, &mut backend)
            .observe((&frame, invalid), &mut trace)
            .is_err()
    );
    assert_eq!(trace.spans.len(), 1);
    assert_eq!(trace.spans[0].name, "render.preparation");
    assert!(!trace.spans[0].success);
}
