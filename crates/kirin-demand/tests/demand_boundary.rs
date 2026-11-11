//! Backward boundary packets and context-specific inspection.
use kirin::prelude::*;
use kirin_demand::OutputDemandContext;
use kirin_demand::{Demand, DemandAnalysis};
use kirin_interpreter::{AnalysisResults, InterpreterError};
use kirin_test_languages::GraphFunctionLanguage;

type TestStage = StageInfo<GraphFunctionLanguage>;

// This test language has scalar signature types; return-site rules determine
// boundary arity independently, as with the graph multi-result fixtures.
const PROGRAM: &str = r#"
stage @test fn @triple(i64, i64, i64) -> i64;
stage @test fn @main(i64, i64, i64) -> i64;
stage @test fn @branch(i64, i64) -> i64;
stage @test fn @inner(i64, i64) -> i64;
stage @test fn @swap(i64, i64) -> i64;

specialize @test fn @triple(i64, i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64, %c: i64) {
    ret %c, %a, %b;
  }
}
specialize @test fn @main(i64, i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64, %c: i64) {
    %x, %y, %z = call.named @triple(%a, %b, %c) -> i64, i64, i64;
    ret %x, %y, %z;
  }
}
specialize @test fn @branch(i64, i64) -> i64 cfg {
  ^entry(%value: i64, %cond: i64) {
    cond_br %cond then=^yes(%value) else=^no(%value);
  }
  ^yes(%x: i64) { ret %x; }
  ^no(%y: i64) { ret %y; }
}
specialize @test fn @inner(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    ret %b;
  }
}
specialize @test fn @swap(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %x = call.named @inner(%a, %b) -> i64;
    %y = call.named @inner(%b, %a) -> i64;
    ret %x, %y;
  }
}
"#;

fn pipeline() -> Pipeline<TestStage> {
    let mut pipeline = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, PROGRAM).unwrap();
    pipeline
}

#[test]
fn output_product_returns_parameter_facts_and_preserves_context_queries() {
    let pipeline = pipeline();
    let outputs: Product<_> = [Demand::Live, Demand::Dead, Demand::Live]
        .into_iter()
        .collect();
    let first = DemandAnalysis::<TestStage>::new(&pipeline)
        .analyze_by_name("test", "main", outputs)
        .unwrap();
    assert_eq!(
        first.response(first.root()).unwrap().as_slice(),
        &[Demand::Dead, Demand::Live, Demand::Live]
    );
    assert!(
        first.contexts().count() >= 2,
        "callee facts remain queryable"
    );

    let outputs: Product<_> = [Demand::Dead, Demand::Live, Demand::Dead]
        .into_iter()
        .collect();
    let second = DemandAnalysis::<TestStage>::new(&pipeline)
        .analyze_by_name("test", "main", outputs)
        .unwrap();
    assert_eq!(
        second.response(second.root()).unwrap().as_slice(),
        &[Demand::Live, Demand::Dead, Demand::Dead]
    );
    assert_ne!(second.root(), first.root());
}

#[test]
fn result_values_preserve_control_roots() {
    let pipeline = pipeline();
    let analyzed = DemandAnalysis::<TestStage>::new(&pipeline)
        .analyze_by_name("test", "branch", [Demand::Dead])
        .unwrap();
    assert_eq!(
        analyzed.response(analyzed.root()).unwrap().as_slice(),
        &[Demand::Dead, Demand::Live]
    );
}

#[test]
fn root_arity_errors_are_reported() {
    let pipeline = pipeline();
    let wrong: Product<Demand> = Product::new();
    assert_eq!(
        DemandAnalysis::<TestStage>::new(&pipeline)
            .with_policy(OutputDemandContext::with_budget(0))
            .analyze_by_name("test", "branch", wrong)
            .err(),
        Some(InterpreterError::ProductArityMismatch {
            expected: 1,
            actual: 0
        }),
    );
}

#[test]
fn call_sites_map_one_callee_parameter_to_different_arguments() {
    let pipeline = pipeline();
    for (outputs, inputs) in [
        ([Demand::Live, Demand::Live], [Demand::Live, Demand::Live]),
        ([Demand::Live, Demand::Dead], [Demand::Dead, Demand::Live]),
        ([Demand::Dead, Demand::Live], [Demand::Live, Demand::Dead]),
    ] {
        let analyzed = DemandAnalysis::<TestStage>::new(&pipeline)
            .analyze_by_name("test", "swap", outputs)
            .unwrap();
        let actual = analyzed.response(analyzed.root()).unwrap();
        assert_eq!(actual.as_slice(), &inputs, "outputs {outputs:?}");
    }
}
