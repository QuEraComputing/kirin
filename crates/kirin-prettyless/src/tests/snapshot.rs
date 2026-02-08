#[test]
fn test_block() {
    let (stage, gs, f) = create_test_function();

    // Use the Document method API for printing IR nodes
    let doc = Document::with_global_symbols(Default::default(), &stage, &gs);
    let arena_doc = doc.print_specialized_function(&f);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc.render_fmt(max_width, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn test_render_specialized_function() {
    let (stage, gs, f) = create_test_function();

    // Test the Document render method
    let mut doc = Document::with_global_symbols(Default::default(), &stage, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_custom_width() {
    let (stage, gs, f) = create_test_function();

    let config = Config::default().with_width(40);
    let mut doc = Document::with_global_symbols(config, &stage, &gs);
    let output = doc.render(&f).unwrap();
    insta::assert_snapshot!(output);
}
