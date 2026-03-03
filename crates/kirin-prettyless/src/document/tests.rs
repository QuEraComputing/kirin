use super::builder::strip_trailing_whitespace;

#[test]
fn test_strip_trailing_whitespace_empty() {
    assert_eq!(strip_trailing_whitespace(""), "\n");
}

#[test]
fn test_strip_trailing_whitespace_no_trailing() {
    assert_eq!(strip_trailing_whitespace("hello\nworld"), "hello\nworld\n");
}

#[test]
fn test_strip_trailing_whitespace_with_trailing() {
    assert_eq!(
        strip_trailing_whitespace("hello   \nworld  \n"),
        "hello\nworld\n"
    );
}

#[test]
fn test_strip_trailing_whitespace_mixed() {
    assert_eq!(
        strip_trailing_whitespace("  indented  \n  also  \n"),
        "  indented\n  also\n"
    );
}
