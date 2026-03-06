use kirin::prelude::*;
use kirin_test_languages::ArithFunctionLanguage;

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
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let parsed = pipeline.parse(input).expect("parse should succeed");
    assert!(!parsed.is_empty(), "should parse at least one function");
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
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let parsed = pipeline.parse(input).expect("parse should succeed");
    assert!(!parsed.is_empty(), "should parse at least one function");
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
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let parsed = pipeline.parse(input).expect("parse should succeed");
    assert!(!parsed.is_empty(), "should parse at least one function");
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
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let parsed = pipeline.parse(input).expect("parse should succeed");
    assert!(!parsed.is_empty(), "should parse at least one function");
}
