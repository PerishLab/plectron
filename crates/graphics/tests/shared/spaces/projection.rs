use graphics::shared::{Extent, Frame, Output, Preparation, Scene, Space, Spatial, Visual};

fn node(id: u64, parent: usize, descendants: usize) -> Spatial {
    Spatial {
        space: Space {
            id,
            translation: [0; 2],
            clip: None,
            scroll: None,
        },
        opacity: 255,
        parent,
        descendants,
    }
}

#[test]
fn topology() {
    let extent = Extent {
        width: 8,
        height: 8,
    };
    let scene = Scene::project(extent, 1, vec![node(1, 0, 1), node(2, 1, 0)]).unwrap();
    assert_eq!(scene.mappings(&Visual::new(extent)).unwrap().len(), 3);
    for nodes in [
        vec![node(1, 1, 0)],
        vec![node(1, 0, 2)],
        vec![node(1, 0, 0), node(2, 1, 0)],
        vec![node(1, 0, 1), node(1, 1, 0)],
    ] {
        assert!(Scene::project(extent, 1, nodes).is_err());
    }
    let mut nested = Vec::new();
    for index in 0..33 {
        nested.push(node(index + 1, index as usize, 32 - index as usize));
    }
    assert!(Scene::project(extent, 1, nested).is_err());
}

#[test]
fn rendering() {
    let extent = Extent {
        width: 100,
        height: 100,
    };
    let scene = Scene::project(extent, 1, vec![node(1, 0, 0)]).unwrap();
    let frame = Frame::new(scene, Visual::new(extent)).unwrap();
    let fault = Preparation::default()
        .prepare(&frame, Output::new(extent, 1.0).unwrap())
        .unwrap_err();
    assert_eq!(
        fault.to_string(),
        "scene traversal left 1 of 2 frame mappings"
    );
}
