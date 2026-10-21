use kirin::prelude::*;
use kirin_constprop::{ConstProp, ConstPropValue};
use kirin_interpreter::{InterpreterError, expect_single};
use kirin_test_languages::ScalarFunctionLanguage;

#[test]
fn constprop_analyzes_i32_addition() {
    let mut pipeline: Pipeline<StageInfo<ScalarFunctionLanguage>> = Pipeline::new();
    ParsePipelineText::parse(
        &mut pipeline,
        r#"
stage @test fn @add(i32, i32) -> i32;
specialize @test fn @add(i32, i32) -> i32 cfg {
  ^entry(%x: i32, %y: i32) {
    %sum = add %x, %y -> i32;
    ret %sum;
  }
}
"#,
    )
    .unwrap();

    let mut analysis = ConstProp::<_, i32, InterpreterError>::new(&pipeline);
    let result = analysis
        .analyze_by_name(
            "test",
            "add",
            [ConstPropValue::Const(2), ConstPropValue::Const(3)],
        )
        .unwrap();
    assert_eq!(
        expect_single::<_, InterpreterError>(result).unwrap(),
        ConstPropValue::Const(5)
    );
}
