use kirin_test_languages::ArithFunctionLanguage;
use kirin_test_utils::roundtrip;

#[test]
fn test_branch_roundtrip() {
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
    roundtrip::assert_pipeline_roundtrip::<ArithFunctionLanguage>(input);
}

#[test]
fn test_conditional_branch_roundtrip() {
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
    roundtrip::assert_pipeline_roundtrip::<ArithFunctionLanguage>(input);
}

#[test]
fn test_branch_with_multiple_args_roundtrip() {
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
    roundtrip::assert_pipeline_roundtrip::<ArithFunctionLanguage>(input);
}

#[test]
fn test_diamond_control_flow_roundtrip() {
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
    roundtrip::assert_pipeline_roundtrip::<ArithFunctionLanguage>(input);
}
