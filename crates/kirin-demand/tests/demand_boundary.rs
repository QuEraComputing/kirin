//! Backward boundary packets and context-specific inspection.
use kirin::prelude::*;
use kirin_demand::OutputDemandContext;
use kirin_demand::{Demand, DemandAnalysis, DemandResult};
use kirin_interpreter::InterpreterError;
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
    let mut analysis = DemandAnalysis::<TestStage>::new(&pipeline);
    let outputs: Product<_> = [Demand::Live, Demand::Dead, Demand::Live]
        .into_iter()
        .collect();
    let inputs = analysis.analyze_by_name("test", "main", outputs).unwrap();
    assert_eq!(
        inputs.as_slice(),
        &[Demand::Dead, Demand::Live, Demand::Live]
    );
    let first = analysis.root().unwrap().clone();
    let details = DemandResult::from_engine(&analysis).unwrap();
    assert_eq!(details.root(), &first);
    assert!(
        details.contexts().count() >= 2,
        "callee facts remain queryable"
    );

    let outputs: Product<_> = [Demand::Dead, Demand::Live, Demand::Dead]
        .into_iter()
        .collect();
    let inputs = analysis.analyze_by_name("test", "main", outputs).unwrap();
    assert_eq!(
        inputs.as_slice(),
        &[Demand::Live, Demand::Dead, Demand::Dead]
    );
    assert_ne!(analysis.root(), Some(&first));
    assert_eq!(analysis.result_value(&first, 0), Some(Demand::Live));
}

#[test]
fn result_values_preserve_control_roots() {
    let pipeline = pipeline();
    let mut analysis = DemandAnalysis::<TestStage>::new(&pipeline);
    let inputs = analysis
        .analyze_by_name("test", "branch", [Demand::Dead])
        .unwrap();
    assert_eq!(inputs.as_slice(), &[Demand::Dead, Demand::Live]);
}

#[test]
fn arity_errors_clear_root_and_do_not_lose_discovered_control_roots() {
    let pipeline = pipeline();
    let mut analysis = DemandAnalysis::<TestStage>::new(&pipeline)
        .with_policy(OutputDemandContext::with_budget(0));
    analysis
        .analyze_by_name("test", "main", [Demand::Live, Demand::Live, Demand::Live])
        .unwrap();
    assert!(analysis.root().is_some());
    let wrong: Product<Demand> = Product::new();
    assert_eq!(
        analysis.analyze_by_name("test", "branch", wrong),
        Err(InterpreterError::ProductArityMismatch {
            expected: 1,
            actual: 0
        }),
    );
    assert!(analysis.root().is_none());
    let outputs: Product<_> = [Demand::Dead].into_iter().collect();
    let inputs = analysis.analyze_by_name("test", "branch", outputs).unwrap();
    assert_eq!(inputs.as_slice(), &[Demand::Dead, Demand::Live]);
}

#[test]
fn call_sites_map_one_callee_parameter_to_different_arguments() {
    let pipeline = pipeline();
    let mut analysis = DemandAnalysis::<TestStage>::new(&pipeline);
    for (outputs, inputs) in [
        ([Demand::Live, Demand::Live], [Demand::Live, Demand::Live]),
        ([Demand::Live, Demand::Dead], [Demand::Dead, Demand::Live]),
        ([Demand::Dead, Demand::Live], [Demand::Live, Demand::Dead]),
    ] {
        let actual = analysis.analyze_by_name("test", "swap", outputs).unwrap();
        assert_eq!(actual.as_slice(), &inputs, "outputs {outputs:?}");
    }
}
