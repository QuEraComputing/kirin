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

#[test]
fn test_strip_trailing_whitespace_only_spaces() {
    // A string that is only whitespace on every line
    assert_eq!(strip_trailing_whitespace("   \n   \n"), "\n\n");
}

#[test]
fn test_strip_trailing_whitespace_single_newline() {
    assert_eq!(strip_trailing_whitespace("\n"), "\n");
}

#[test]
fn test_strip_trailing_whitespace_tabs() {
    assert_eq!(
        strip_trailing_whitespace("hello\t\t\nworld\t"),
        "hello\nworld\n"
    );
}

#[test]
fn test_strip_trailing_whitespace_no_final_newline() {
    // Input without trailing newline still gets one appended per line
    assert_eq!(strip_trailing_whitespace("abc"), "abc\n");
}

#[test]
fn test_strip_trailing_whitespace_multiple_blank_lines() {
    assert_eq!(strip_trailing_whitespace("a\n\n\nb"), "a\n\n\nb\n");
}
