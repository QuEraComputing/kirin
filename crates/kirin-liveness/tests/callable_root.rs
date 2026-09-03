//! Acceptance tests for the common callable-root protocol used by both
//! backward engines.

use kirin::prelude::*;
use kirin_interpreter::{
    Body, Callee, CrossStageLinker, FunctionTarget, InterpreterError, Linker, SameStageLinker,
};
use kirin_liveness::{Demand, DenseLiveness};
use kirin_test_languages::GraphFunctionLanguage;

type TestStage = StageInfo<GraphFunctionLanguage>;

const BLOCK_PROGRAM: &str = r#"
stage @test fn @linear(i64) -> i64;
stage @test fn @main(i64) -> i64;

specialize @test fn @linear(i64) -> i64 ^body(%x: i64) {
  %y = neg %x -> i64;
  ret %y;
}

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    %y = call.named @linear(%x) -> i64;
    ret %y;
  }
}
"#;

const CROSS_STAGE_PROGRAM: &str = r#"
stage @source fn @linear(i64) -> i64;
stage @lowered fn @linear(i64) -> i64;

specialize @lowered fn @linear(i64) -> i64 ^body(%x: i64) {
  ret %x;
}
"#;

const GRAPH_PROGRAM: &str = r#"
stage @test fn @directed(i64) -> i64;
stage @test fn @undirected(i64) -> i64;

specialize @test fn @directed(i64) -> i64 digraph ^graph(%x: i64) {
  %y = neg %x -> i64;
  yield %y;
}

specialize @test fn @undirected(i64) -> i64 ungraph ^graph(%x: i64) {
  %y = neg %x -> i64;
}
"#;

const AMBIGUOUS_PROGRAM: &str = r#"
stage @test fn @ambiguous(i64, i64) -> i64;

specialize @test fn @ambiguous(i64) -> i64 ^first(%x: i64) {
  ret %x;
}

specialize @test fn @ambiguous(i64, i64) -> i64 ^second(%x: i64, %y: i64) {
  ret %x;
}
"#;

fn parse(program: &str) -> Pipeline<TestStage> {
    let mut pipeline = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
    pipeline
}

fn function_callee(pipeline: &Pipeline<TestStage>, name: &str) -> (CompileStage, Callee) {
    let stage = pipeline.stage_by_name("test").expect("stage exists");
    let function = pipeline
        .lookup_function_by_name(name)
        .expect("function exists");
    (stage, Callee::Function(function))
}

fn all_callee_variants(pipeline: &Pipeline<TestStage>) -> (CompileStage, [Callee; 4]) {
    let stage = pipeline.stage_by_name("test").expect("stage exists");
    let info = pipeline.stage(stage).expect("stage info exists");
    let symbol = info.symbol_table().lookup("linear").expect("symbol exists");
    let function = pipeline
        .lookup_function_by_name("linear")
        .expect("function exists");
    let staged = pipeline
        .resolve_staged_function("linear", stage)
        .expect("staged function exists");
    let specialized = staged
        .get_info(info)
        .expect("staged function info exists")
        .unique_live_specialization()
        .expect("one live specialization");
    (
        stage,
        [
            Callee::Named(symbol),
            Callee::Function(function),
            Callee::Staged(staged),
            Callee::Specialized(specialized),
        ],
    )
}

#[test]
fn every_callee_variant_uses_the_same_backward_root_protocol() {
    let pipeline = parse(BLOCK_PROGRAM);
    let (stage, callees) = all_callee_variants(&pipeline);

    for callee in callees {
        let mut demand = Demand::<TestStage>::new(&pipeline);
        let demand_scope = demand.analyze(stage, callee).expect("demand succeeds");
        assert!(matches!(demand_scope, (_, Body::Block(_))));

        let mut dense = DenseLiveness::<TestStage>::new(&pipeline);
        let dense_scope = dense.analyze(stage, callee).expect("dense succeeds");
        assert_eq!(dense_scope, demand_scope);
    }
}

#[derive(Clone, Copy)]
struct RejectingLinker;

impl Linker<TestStage> for RejectingLinker {
    fn resolve(
        &self,
        _pipeline: &Pipeline<TestStage>,
        _caller_stage: CompileStage,
        _callee: &Callee,
    ) -> Result<FunctionTarget, InterpreterError> {
        Err(InterpreterError::Custom("rejecting linker reached"))
    }
}

#[test]
fn no_callee_variant_bypasses_the_configured_linker() {
    let pipeline = parse(BLOCK_PROGRAM);
    let (stage, callees) = all_callee_variants(&pipeline);

    for callee in callees {
        let demand_error = Demand::<TestStage>::new(&pipeline)
            .with_linker(RejectingLinker)
            .analyze(stage, callee)
            .expect_err("demand must use the linker");
        assert_eq!(
            demand_error,
            InterpreterError::Custom("rejecting linker reached")
        );

        let dense_error = DenseLiveness::<TestStage>::new(&pipeline)
            .with_linker(RejectingLinker)
            .analyze(stage, callee)
            .expect_err("dense must use the linker");
        assert_eq!(
            dense_error,
            InterpreterError::Custom("rejecting linker reached")
        );
    }
}

#[test]
fn cross_stage_linking_discovers_the_body_at_the_target_stage() {
    let pipeline = parse(CROSS_STAGE_PROGRAM);
    let source = pipeline.stage_by_name("source").expect("source exists");
    let lowered = pipeline.stage_by_name("lowered").expect("lowered exists");
    let function = pipeline
        .lookup_function_by_name("linear")
        .expect("function exists");
    let callee = Callee::Function(function);

    assert!(matches!(
        Demand::<TestStage>::new(&pipeline).analyze(source, callee),
        Err(InterpreterError::MissingSpecialization(_))
    ));
    assert!(matches!(
        DenseLiveness::<TestStage>::new(&pipeline).analyze(source, callee),
        Err(InterpreterError::MissingSpecialization(_))
    ));

    let demand_scope = Demand::<TestStage>::new(&pipeline)
        .with_linker(CrossStageLinker)
        .analyze(source, callee)
        .expect("cross-stage demand succeeds");
    let mut dense = DenseLiveness::<TestStage>::new(&pipeline).with_linker(CrossStageLinker);
    let dense_scope = dense
        .analyze(source, callee)
        .expect("cross-stage dense succeeds");
    assert_eq!(demand_scope.0, lowered);
    assert!(matches!(demand_scope.1, Body::Block(_)));
    assert_eq!(dense_scope, demand_scope);

    // Dense analysis rebuilds solver state between roots; configuration state
    // must survive that reset.
    assert_eq!(
        dense
            .analyze(source, callee)
            .expect("custom linker survives dense reset"),
        dense_scope
    );
}

#[derive(Clone, Copy)]
struct FixedTargetLinker(FunctionTarget);

impl Linker<TestStage> for FixedTargetLinker {
    fn resolve(
        &self,
        _pipeline: &Pipeline<TestStage>,
        _caller_stage: CompileStage,
        _callee: &Callee,
    ) -> Result<FunctionTarget, InterpreterError> {
        Ok(self.0)
    }
}

#[test]
fn a_resolved_non_callable_definition_is_reported_by_shared_body_discovery() {
    let pipeline = parse(BLOCK_PROGRAM);
    let (stage, callee) = function_callee(&pipeline, "linear");
    let mut target = SameStageLinker
        .resolve(&pipeline, stage, &callee)
        .expect("function resolves");
    let info = pipeline.stage(stage).expect("stage info exists");
    let body = match target.definition.definition(info) {
        GraphFunctionLanguage::LinearFunction { body, .. } => *body,
        other => panic!("expected linear function, got {other:?}"),
    };
    let non_callable = body
        .statements(info)
        .next()
        .expect("body contains an arithmetic statement");
    target.definition = non_callable;

    let demand_error = Demand::<TestStage>::new(&pipeline)
        .with_linker(FixedTargetLinker(target))
        .analyze(stage, callee)
        .expect_err("non-callable definition must fail");
    assert_eq!(demand_error, InterpreterError::NotCallable(non_callable));

    let dense_error = DenseLiveness::<TestStage>::new(&pipeline)
        .with_linker(FixedTargetLinker(target))
        .analyze(stage, callee)
        .expect_err("non-callable definition must fail");
    assert_eq!(dense_error, InterpreterError::NotCallable(non_callable));
}

#[test]
fn unsupported_graph_roots_fail_instead_of_producing_empty_facts() {
    let pipeline = parse(GRAPH_PROGRAM);

    for name in ["directed", "undirected"] {
        let (stage, callee) = function_callee(&pipeline, name);
        let demand_error = Demand::<TestStage>::new(&pipeline)
            .analyze(stage, callee)
            .expect_err("graph demand is not implemented");
        assert!(matches!(
            demand_error,
            InterpreterError::NoDefaultWalker(Body::DiGraph(_) | Body::UnGraph(_))
        ));

        let dense_error = DenseLiveness::<TestStage>::new(&pipeline)
            .analyze(stage, callee)
            .expect_err("graph liveness is not implemented");
        assert!(matches!(
            dense_error,
            InterpreterError::NoDefaultWalker(Body::DiGraph(_) | Body::UnGraph(_))
        ));
    }
}

#[test]
fn linker_resolution_errors_propagate_from_both_backward_engines() {
    let pipeline = parse(BLOCK_PROGRAM);
    let stage = pipeline.stage_by_name("test").expect("stage exists");
    let missing = Callee::Named(Symbol::from(usize::MAX));

    assert!(matches!(
        Demand::<TestStage>::new(&pipeline).analyze(stage, missing),
        Err(InterpreterError::MissingCallSymbol(_))
    ));
    assert!(matches!(
        DenseLiveness::<TestStage>::new(&pipeline).analyze(stage, missing),
        Err(InterpreterError::MissingCallSymbol(_))
    ));

    let ambiguous_pipeline = parse(AMBIGUOUS_PROGRAM);
    let (ambiguous_stage, ambiguous) = function_callee(&ambiguous_pipeline, "ambiguous");
    assert!(matches!(
        Demand::<TestStage>::new(&ambiguous_pipeline).analyze(ambiguous_stage, ambiguous),
        Err(InterpreterError::AmbiguousSpecialization { count: 2, .. })
    ));
    assert!(matches!(
        DenseLiveness::<TestStage>::new(&ambiguous_pipeline).analyze(ambiguous_stage, ambiguous),
        Err(InterpreterError::AmbiguousSpecialization { count: 2, .. })
    ));
}
