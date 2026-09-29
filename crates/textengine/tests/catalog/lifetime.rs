use super::*;

#[test]
fn requests() {
    let catalog = ::fixtures::catalog();
    let original = request(&["lato"]);
    let before = catalog.shape("fi", &original, 0).unwrap();
    let mut changed = original.clone();
    assert!(Arc::ptr_eq(&original.families, &changed.families));
    Arc::make_mut(&mut changed.families)[0] = "toy".into();
    let after = catalog.shape("fi", &changed, 0).unwrap();
    assert_ne!(before.spans[0].font.face, after.spans[0].font.face);
    let wire = serde_json::to_value(&original).unwrap();
    assert_eq!(wire["families"], serde_json::json!(["lato"]));
    let replay: Request = serde_json::from_value(wire).unwrap();
    assert!(!Arc::ptr_eq(&original.families, &replay.families));
    assert_eq!(original, replay);
    let replayed = catalog.shape("fi", &replay, 0).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(replayed).unwrap()
    );
    assert_eq!(original.families.as_ref(), ["lato"]);
    assert_eq!(Request::default().families.as_ref(), ["toy"]);
}

#[test]
fn ownership() {
    let bytes: Arc<[u8]> = Arc::from(::fixtures::bytes("Lato-Regular.ttf"));
    let lease = Lease::new(bytes).unwrap();
    let catalog = Catalog::new();
    let registration = catalog
        .register("application", Weight::Normal, Face::Owned(lease.clone()))
        .unwrap();
    let request = request(&["application"]);
    let run = catalog.shape("fi", &request, 0).unwrap();
    let prepared = catalog.prepare("fi", &request, 0).unwrap();
    let font = &run.spans[0].font;
    let outline = font.outline(run.glyphs[0].id).unwrap();
    assert!(catalog.unregister(&registration).unwrap());
    assert_eq!(
        catalog.shape("fi", &request, 0).unwrap().spans[0].font.face,
        Face::Missing
    );
    assert_eq!(font.outline(run.glyphs[0].id).unwrap(), outline);
    let wire = serde_json::to_string(&run).unwrap();
    let replay: textengine::Run = serde_json::from_str(&wire).unwrap();
    replay.validate("fi").unwrap();
    assert_ne!(replay.spans[0].font.face, font.face);
    assert_eq!(
        replay.spans[0].font.outline(replay.glyphs[0].id).unwrap(),
        outline
    );
    catalog.close();
    assert!(
        catalog
            .register("application", Weight::Normal, Face::Owned(lease))
            .is_err()
    );
    let split = prepared.shape(1..2).unwrap();
    assert_eq!(split.spans[0].font.face, font.face);
    assert_eq!(split.glyphs[0].id, font.index('i'));
    let intact = prepared.shape(0..2).unwrap();
    assert_eq!(intact.glyphs[0].id, run.glyphs[0].id);
    assert_eq!(font.outline(run.glyphs[0].id).unwrap(), outline);
}
