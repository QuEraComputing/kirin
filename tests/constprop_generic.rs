use kirin::prelude::*;
use kirin_constprop::{ConstProp, ConstPropValue};
use kirin_interpreter::{AnalysisResults, InterpreterError, expect_single};
use kirin_test_languages::ScalarFunctionLanguage;

#[test]
fn constprop_analyzes_i32_addition() {
    let mut program: Program<StageInfo<ScalarFunctionLanguage>> = Program::new();
    ParseProgramText::parse(
        &mut program,
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

    let analysis = ConstProp::<_, i32, InterpreterError>::new(&program);
    let analyzed = analysis
        .analyze_by_name(
            "test",
            "add",
            [ConstPropValue::Const(2), ConstPropValue::Const(3)],
        )
        .unwrap();
    let result = analyzed.response(analyzed.root()).unwrap();
    assert_eq!(
        expect_single::<_, InterpreterError>(result).unwrap(),
        ConstPropValue::Const(5)
    );
}
