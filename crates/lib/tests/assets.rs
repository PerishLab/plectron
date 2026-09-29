use graphics::shared::{Backend, Cpu, Extent};
use plectron::resource::Services;
use webengine::{
    Page,
    resource::{Kind, Provider, Request, address},
};

#[test]
fn network() {
    let mut services = Services::default();
    let service = services.serve(&fixtures::root()).unwrap();
    services.scheme(service, "app://assets/").unwrap();
    let remote = services.inspect()[0].address.clone();
    let font = address(&format!("{remote}/fonts/SC.ttf")).unwrap();
    assert!(services.get(&font).is_err());
    let response = services.request(&Request::new(font, Kind::Font)).unwrap();
    assert_eq!(response.mime, "font/ttf");
    assert_eq!(response.body, fixtures::bytes("SC.ttf"));
    let mut images = Vec::new();
    for base in [remote, "app://assets".into()] {
        let mut page = Page::new(
            &format!("{base}/network/index.html"),
            Extent {
                width: 720,
                height: 640,
            },
        )
        .unwrap();
        page.load(&mut services).unwrap();
        let snapshot = page.inspect();
        let fonts: Vec<_> = snapshot
            .resources
            .iter()
            .filter(|resource| resource.mime == "font/ttf")
            .collect();
        assert_eq!(fonts.len(), 4);
        assert!(fonts.iter().all(|font| font.bytes > 65_536));
        let frame = page.frame().unwrap();
        let pixels = Cpu.render(&frame).unwrap();
        page.acknowledge(frame.id()).unwrap();
        page.evaluate("document.getElementById('swap').dispatchEvent('click', '');")
            .unwrap();
        page.render().unwrap();
        let changed = page.frame().unwrap();
        assert_ne!(Cpu.render(&changed).unwrap(), pixels);
        assert_eq!(
            page.inspect()
                .resources
                .iter()
                .filter(|resource| resource.mime == "font/ttf")
                .count(),
            4
        );
        page.close();
        assert_eq!(Cpu.render(&frame).unwrap(), pixels);
        images.push(pixels);
    }
    services.close(service).unwrap();
    assert_eq!(images[0], images[1]);
}
