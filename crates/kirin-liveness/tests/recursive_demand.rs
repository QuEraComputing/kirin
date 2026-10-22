use kirin::prelude::*;
use kirin_liveness::{Demand, DemandAnalysis};
use kirin_test_languages::GraphFunctionLanguage;

#[test]
fn recursive_calls_propagate_late_parameter_values() {
    // The cycle reaches its parameter demand only through the base return.
    // Both a self edge and a two-function cycle must wake suspended callers.
    for callee in ["first", "second"] {
        let source = format!(
            r#"
stage @test fn @first(i64, i64, i64) -> i64;
stage @test fn @second(i64, i64, i64) -> i64;
specialize @test fn @first(i64, i64, i64) -> i64 cfg {{
  ^entry(%a: i64, %unused: i64, %cond: i64) {{
    cond_br %cond then=^base(%a) else=^recurse(%a, %unused, %cond);
  }}
  ^base(%value: i64) {{ ret %value; }}
  ^recurse(%x: i64, %y: i64, %c: i64) {{
    %result = call.named @{callee}(%x, %y, %c) -> i64;
    ret %result;
  }}
}}
specialize @test fn @second(i64, i64, i64) -> i64 cfg {{
  ^entry(%a: i64, %unused: i64, %cond: i64) {{
    %result = call.named @first(%a, %unused, %cond) -> i64;
    ret %result;
  }}
}}
"#
        );
        let mut pipeline: Pipeline<StageInfo<GraphFunctionLanguage>> = Pipeline::new();
        ParsePipelineText::parse(&mut pipeline, &source).unwrap();
        let mut analysis = DemandAnalysis::<StageInfo<GraphFunctionLanguage>>::new(&pipeline);
        assert_eq!(
            analysis
                .analyze_by_name("test", "second", [Demand::Live])
                .unwrap()
                .as_slice(),
            &[Demand::Live, Demand::Dead, Demand::Live]
        );
        assert_eq!(analysis.contexts().count(), 2);
    }
}
