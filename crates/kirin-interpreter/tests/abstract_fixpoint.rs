use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{
    AbstractInterpreter, AnalysisResult, InterpreterError, StageAccess, WideningStrategy,
};
use kirin_interval::Interval;
use kirin_ir::*;
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::dump_function;
use kirin_test_utils::ir_fixtures::{
    build_add_one, build_branch_fork_program, build_loop_program, build_select_program,
};

// ---------------------------------------------------------------------------
// IR snapshot tests
// ---------------------------------------------------------------------------

#[test]
fn test_branch_fork_ir_snapshot() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_branch_fork_program(&mut pipeline, stage_id);
    let ir = dump_function(spec_fn, &pipeline, stage_id);
    insta::assert_snapshot!(ir);
}

#[test]
fn test_loop_convergence_ir_snapshot() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_loop_program(&mut pipeline, stage_id);
    let ir = dump_function(spec_fn, &pipeline, stage_id);
    insta::assert_snapshot!(ir);
}

// ---------------------------------------------------------------------------
// Test: Straight-line constant propagation
// ---------------------------------------------------------------------------

/// Build `c1 = constant 10; c2 = constant 32; y = add c1, c2; return y`
/// Run through AbstractInterpreter and verify return value.
#[test]
fn test_abstract_interp_constants() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(10));
        let c2 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(32));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, c1.result, c2.result);
        let ret = Return::<ArithType>::new(b, vec![add.result.into()]);

        let block = b.block().stmt(c1).stmt(c2).stmt(add).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    });

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[])
        .unwrap();
    assert_eq!(result.return_value(), Some(&Interval::constant(42)));
}

// ---------------------------------------------------------------------------
// Test: Branching with Fork (undecidable condition)
// ---------------------------------------------------------------------------

/// Pass Interval(-10, 10) which spans zero — is_truthy returns None → Fork.
/// Verify both branches explored, return value is join of both paths.
#[test]
fn test_abstract_interp_branch_fork() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_branch_fork_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-10, 10)])
        .unwrap();

    // The return value is the join of:
    // - neg path: neg([-10, 10]) = [-10, 10]
    // - pos path: [-10, 10]
    // join = [-10, 10]
    let ret = result.return_value().unwrap();
    assert_eq!(*ret, Interval::new(-10, 10));
}

// ---------------------------------------------------------------------------
// Test: Loop with back-edge, block args, and worklist convergence
// ---------------------------------------------------------------------------

/// This tests that:
/// - Block arguments propagate values across control flow edges
/// - The worklist converges with widening
/// - Back-edge state propagation works correctly
#[test]
fn test_abstract_interp_loop_convergence() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_max_iterations(100);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    // The analysis should converge and produce a non-empty return value.
    // The loop variable widens across iterations, so we can't assert exact value.
    let ret = result.return_value().unwrap();
    assert!(
        !ret.is_empty(),
        "return value should not be bottom after loop convergence"
    );
}

// ---------------------------------------------------------------------------
// Test: Summary caching via call()
// ---------------------------------------------------------------------------

/// Call the same function twice and verify the summary cache is populated
/// after the first call and reused on the second.
#[test]
fn test_abstract_interp_call_caches_summary() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(7));
        let c2 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(3));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, c1.result, c2.result);
        let ret = Return::<ArithType>::new(b, vec![add.result.into()]);

        let block = b.block().stmt(c1).stmt(c2).stmt(add).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    });

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    // First call — runs the analysis
    let result1 = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[])
        .unwrap();
    assert_eq!(result1.return_value(), Some(&Interval::constant(10)));

    // Summary should be cached (args subsumed)
    assert!(
        interp
            .in_stage::<CompositeLanguage>()
            .summary(spec_fn, &[])
            .is_some()
    );
    assert_eq!(
        interp
            .in_stage::<CompositeLanguage>()
            .summary(spec_fn, &[])
            .unwrap()
            .return_value(),
        Some(&Interval::constant(10))
    );

    // Second call with same args — returns cached summary
    let result2 = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[])
        .unwrap();
    assert_eq!(result2.return_value(), Some(&Interval::constant(10)));
}

#[test]
fn test_abstract_interp_in_stage_chain() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_branch_fork_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-10, 10)])
        .unwrap();

    assert_eq!(result.return_value(), Some(&Interval::new(-10, 10)));
}

#[test]
fn test_abstract_interp_with_stage_chain() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_branch_fork_program(&mut pipeline, stage_id);
    let stage = pipeline.stage(stage_id).unwrap();

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .with_stage(stage)
        .analyze(spec_fn, &[Interval::new(-10, 10)])
        .unwrap();

    assert_eq!(result.return_value(), Some(&Interval::new(-10, 10)));
}

// ---------------------------------------------------------------------------
// Widening strategy tests
// ---------------------------------------------------------------------------

#[test]
fn test_abstract_widening_never() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::Never)
            .with_max_iterations(50);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-5, 5)]);
    assert!(
        matches!(result, Err(InterpreterError::FuelExhausted)),
        "expected FuelExhausted without widening, got: {result:?}"
    );
}

#[test]
fn test_abstract_widening_delayed() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::Delayed(3))
            .with_max_iterations(100);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    let ret = result.return_value().unwrap();
    assert!(
        !ret.is_empty(),
        "return value should not be bottom after delayed widening"
    );
}

#[test]
fn test_abstract_widening_all_joins() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_max_iterations(100);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    let ret = result.return_value().unwrap();
    assert!(
        !ret.is_empty(),
        "return value should not be bottom with AllJoins widening"
    );
}

// ---------------------------------------------------------------------------
// Summary management tests
// ---------------------------------------------------------------------------

#[test]
fn test_abstract_fixed_summary() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let fixed_result = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::new(100, 200)),
    );
    interp
        .in_stage::<CompositeLanguage>()
        .insert_summary(spec_fn)
        .fixed(fixed_result);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 10)])
        .unwrap();
    assert_eq!(result.return_value(), Some(&Interval::new(100, 200)));

    let result2 = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::top()])
        .unwrap();
    assert_eq!(result2.return_value(), Some(&Interval::new(100, 200)));
}

#[test]
fn test_abstract_summary_invalidation_and_gc() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 10)])
        .unwrap();
    assert_eq!(result.return_value(), Some(&Interval::new(1, 11)));

    assert!(
        interp
            .in_stage::<CompositeLanguage>()
            .summary(spec_fn, &[Interval::new(0, 10)])
            .is_some()
    );

    let count = interp
        .in_stage::<CompositeLanguage>()
        .invalidate_summary(spec_fn);
    assert!(count > 0, "expected at least one invalidated entry");
    assert!(
        interp
            .in_stage::<CompositeLanguage>()
            .summary(spec_fn, &[Interval::new(0, 10)])
            .is_none()
    );

    interp.gc_summaries();

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 10)])
        .unwrap();
    assert_eq!(result.return_value(), Some(&Interval::new(1, 11)));
}

#[test]
fn test_abstract_remove_summary() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::constant(5)])
        .unwrap();
    assert!(
        interp
            .in_stage::<CompositeLanguage>()
            .summary(spec_fn, &[Interval::constant(5)])
            .is_some()
    );

    assert!(
        interp
            .in_stage::<CompositeLanguage>()
            .remove_summary(spec_fn)
    );
    assert!(
        interp
            .in_stage::<CompositeLanguage>()
            .summary(spec_fn, &[Interval::constant(5)])
            .is_none()
    );
    assert!(
        !interp
            .in_stage::<CompositeLanguage>()
            .remove_summary(spec_fn)
    );
}

// ---------------------------------------------------------------------------
// Analysis result query tests
// ---------------------------------------------------------------------------

#[test]
fn test_abstract_analysis_result_queries() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_select_program(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    let visited: Vec<_> = result.visited_blocks().collect();
    assert!(
        visited.len() >= 3,
        "expected at least 3 visited blocks, got {}",
        visited.len()
    );

    let ret = result.return_value().unwrap();
    assert!(!ret.is_empty(), "return value should not be bottom");
}

#[test]
fn test_abstract_analysis_result_ssa_values() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec_fn, c1_result, c2_result, add_result) =
        pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
            let sf = b.staged_function().new().unwrap();
            let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(7));
            let c1_result = c1.result;
            let c2 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(3));
            let c2_result = c2.result;
            let add = kirin_arith::Arith::<ArithType>::op_add(b, c1.result, c2.result);
            let add_result = add.result;
            let ret = Return::<ArithType>::new(b, vec![add.result.into()]);

            let block = b.block().stmt(c1).stmt(c2).stmt(add).terminator(ret).new();
            let region = b.region().add_block(block).new();
            let func_body = FunctionBody::<ArithType>::new(
                b,
                region,
                kirin_ir::Signature::new(vec![], ArithType::default(), ()),
            );
            let spec_fn = b
                .specialize()
                .staged_func(sf)
                .body(func_body)
                .new()
                .unwrap();
            (spec_fn, c1_result, c2_result, add_result)
        });

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[])
        .unwrap();

    assert_eq!(
        result.ssa_value(c1_result.into()),
        Some(&Interval::constant(7))
    );
    assert_eq!(
        result.ssa_value(c2_result.into()),
        Some(&Interval::constant(3))
    );
    assert_eq!(
        result.ssa_value(add_result.into()),
        Some(&Interval::constant(10))
    );
    assert_eq!(result.return_value(), Some(&Interval::constant(10)));
}
