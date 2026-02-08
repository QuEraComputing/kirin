#[test]
fn test_write_to_vec() {
    let (stage, gs, f) = create_test_function();

    let mut doc = Document::with_global_symbols(Default::default(), &stage, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_write_with_config() {
    let (stage, gs, f) = create_test_function();

    let config = Config::default().with_width(40);
    let mut doc = Document::with_global_symbols(config, &stage, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}
