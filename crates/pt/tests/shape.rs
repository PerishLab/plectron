#[test]
fn fields() {
    let artifact = pt::compile("<template tag=\"div\" @click={activate}><button ref=\"action\" @click={activate}></button><text ref=\"value\"/></template>").unwrap();
    assert_eq!(artifact.shape.host, "div");
    assert_eq!(artifact.shape.refs["action"], "button");
    assert_eq!(artifact.shape.refs["value"], "text");
    assert_eq!(
        artifact.shape.handlers.iter().collect::<Vec<_>>(),
        ["activate"]
    );
    assert!(artifact.shape.text);
    let declaration = artifact.shape.declaration();
    assert!(declaration.contains("action: ReturnType<D[\"createElement\"]>"));
    assert!(declaration.contains("value: ReturnType<D[\"createTextNode\"]>"));
    assert!(declaration.contains("activate:"));
    assert!(!declaration.contains("dispose") && !declaration.contains("mount"));
}

#[test]
fn absence() {
    let artifact =
        pt::compile("<template tag=\"div\"><span ref=\"body\"></span></template>").unwrap();
    assert!(!artifact.shape.text);
    assert!(artifact.shape.handlers.is_empty());
    let declaration = artifact.shape.declaration();
    assert!(!declaration.contains("listen"));
    assert!(!declaration.contains("createTextNode"));
    assert!(!declaration.contains("Record<string"));
    assert!(
        pt::compile("<template tag=\"div\">hello</template>")
            .unwrap()
            .shape
            .text
    );
}

#[test]
fn rejection() {
    for source in [
        "<template tag=\"div\"><text ref=\"same\"/><span ref=\"same\"></span></template>",
        "<template tag=\"div\"><widget ref=\"value\"/></template>",
        "<template tag=\"div\">{value}</template>",
    ] {
        assert!(pt::compile(source).is_err());
    }
}
