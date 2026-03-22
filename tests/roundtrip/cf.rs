use kirin::prelude::*;
use kirin_test_languages::ArithFunctionLanguage;

/// Parse the CF input and verify structural properties.
/// Note: CF branch tests use parse-only (not full roundtrip) because
/// `Successor::Display` outputs raw block IDs (`^0`) while block headers
/// use symbolic names, causing a roundtrip mismatch.
fn parse_cf_program(input: &str) -> (Pipeline<StageInfo<ArithFunctionLanguage>>, Vec<Function>) {
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let parsed = pipeline.parse(input).expect("parse should succeed");
    assert!(!parsed.is_empty(), "should parse at least one function");
    (pipeline, parsed)
}

#[test]
fn test_branch_parse() {
    let input = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    br ^exit(%x);
  }
  ^exit(%r: i64) {
    ret %r;
  }
}
"#;
    let (pipeline, parsed) = parse_cf_program(input);
    assert_eq!(parsed.len(), 1, "should parse exactly one function");

    let printed = pipeline.sprint();
    assert!(
        printed.contains("^entry"),
        "should contain entry block label"
    );
    assert!(printed.contains("^exit"), "should contain exit block label");
    assert!(
        printed.contains("ret %r"),
        "should contain return terminator"
    );
}

#[test]
fn test_conditional_branch_parse() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    cond_br %cond then=^then(%x) else=^else(%x);
  }
  ^then(%a: i64) {
    ret %a;
  }
  ^else(%b: i64) {
    %neg = neg %b -> i64;
    ret %neg;
  }
}
"#;
    let (pipeline, parsed) = parse_cf_program(input);
    assert_eq!(parsed.len(), 1, "should parse exactly one function");

    let printed = pipeline.sprint();
    assert!(printed.contains("^entry"), "should contain entry block");
    assert!(printed.contains("^then"), "should contain then block");
    assert!(printed.contains("^else"), "should contain else block");
    assert!(
        printed.contains("cond_br"),
        "should contain conditional branch"
    );
    assert!(printed.contains("neg"), "should contain neg operation");
}

#[test]
fn test_branch_with_multiple_args() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %y: i64) {
    br ^target(%x, %y);
  }
  ^target(%a: i64, %b: i64) {
    %sum = add %a, %b -> i64;
    ret %sum;
  }
}
"#;
    let (pipeline, parsed) = parse_cf_program(input);
    assert_eq!(parsed.len(), 1, "should parse exactly one function");

    let printed = pipeline.sprint();
    assert!(printed.contains("^entry"), "should contain entry block");
    assert!(printed.contains("^target"), "should contain target block");
    assert!(printed.contains("add"), "should contain add operation");
}

#[test]
fn test_diamond_control_flow() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    cond_br %cond then=^left(%x) else=^right(%x);
  }
  ^left(%a: i64) {
    %doubled = add %a, %a -> i64;
    br ^merge(%doubled);
  }
  ^right(%b: i64) {
    %negated = neg %b -> i64;
    br ^merge(%negated);
  }
  ^merge(%result: i64) {
    ret %result;
  }
}
"#;
    let (pipeline, parsed) = parse_cf_program(input);
    assert_eq!(parsed.len(), 1, "should parse exactly one function");

    let printed = pipeline.sprint();
    // Verify all 4 blocks are present
    assert!(printed.contains("^entry"), "should contain entry block");
    assert!(printed.contains("^left"), "should contain left block");
    assert!(printed.contains("^right"), "should contain right block");
    assert!(printed.contains("^merge"), "should contain merge block");
    // Verify both terminator types
    assert!(
        printed.contains("cond_br"),
        "should contain conditional branch"
    );
    // Verify non-terminator operations
    assert!(printed.contains("add"), "should contain add operation");
    assert!(printed.contains("neg"), "should contain neg operation");
}
