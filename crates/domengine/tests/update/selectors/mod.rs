mod animation;
use domengine::{
    Sheet,
    style::{Inherited, Resolver, Subject},
};
use shared::cache::{Command, Manager};

fn subject(id: &str, classes: &str) -> Subject {
    Subject {
        tag: Some("p".into()),
        id: Some(id.into()),
        classes: Some(classes.into()),
        inline: None,
        direction: None,
    }
}

#[test]
fn sharing() {
    let rules =
        "p { width:24px; } .red { color:#ff0000; } .big { height:32px; } #special { width:48px; }";
    let manager = Manager::default();
    let sheet = Sheet::parse(rules).unwrap();
    let mut resolver = Resolver::managed(Sheet::parse(rules).unwrap(), &manager).unwrap();
    for index in 0..64 {
        let classes = if index % 2 == 0 {
            "red big red unused"
        } else {
            "big red other"
        };
        let subject = subject(&format!("node{index}"), classes);
        let expected = sheet
            .matching(&subject)
            .unwrap()
            .compute(Inherited::default())
            .unwrap();
        assert_eq!(
            *resolver.resolve(subject, Inherited::default()).unwrap(),
            expected
        );
    }
    resolver.publish();
    let matching = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "matching")
        .unwrap();
    assert_eq!(matching.stats.inserted, 1);
    for subject in [
        subject("special", "red big"),
        subject("node", "red"),
        subject("node", "big"),
        subject("node", "irrelevant"),
    ] {
        let expected = sheet
            .matching(&subject)
            .unwrap()
            .compute(Inherited::default())
            .unwrap();
        assert_eq!(
            *resolver.resolve(subject, Inherited::default()).unwrap(),
            expected
        );
    }
    for cache in manager.inspect() {
        manager.command(cache.id, Command::Clear).unwrap();
    }
    let original = subject("node", "irrelevant");
    resolver
        .resolve(original.clone(), Inherited::default())
        .unwrap();
    resolver.replace(Sheet::parse("#node { width:90px; } .irrelevant { height:50px; }").unwrap());
    let changed = resolver
        .resolve(original.clone(), Inherited::default())
        .unwrap();
    assert_eq!((changed.width, changed.height), (Some(90), Some(50)));
    let mut inline = original;
    inline.inline = Some("width:12px;".into());
    assert_eq!(
        resolver
            .resolve(inline.clone(), Inherited::default())
            .unwrap()
            .width,
        Some(12)
    );
    inline.direction = Some("invalid".into());
    assert!(resolver.resolve(inline, Inherited::default()).is_err());
}
