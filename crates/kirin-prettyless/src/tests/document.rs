#[test]
fn test_document_list_empty() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);

    let items: Vec<i32> = vec![];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"");
}

#[test]
fn test_document_list_single() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);

    let items = vec![42];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"42");
}

#[test]
fn test_document_list_multiple() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let doc = Document::new(Default::default(), &stage);

    let items = vec![1, 2, 3];
    let result = doc.list(items.iter(), ", ", |i| doc.text(format!("{}", i)));
    let mut buf = String::new();
    result.render_fmt(80, &mut buf).unwrap();
    insta::assert_snapshot!(buf, @"1, 2, 3");
}

#[test]
fn test_document_indent() {
    let stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let config = Config::default().with_tab_spaces(4);
    let doc = Document::new(config, &stage);

    // Create indented content with line breaks
    let inner = doc.text("hello") + doc.line() + doc.text("world");
    let result = doc.indent(inner);
    let mut buf = String::new();
    result.render_fmt(5, &mut buf).unwrap(); // Force line break
    insta::assert_snapshot!(buf);
}
