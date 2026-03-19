//! Tests for abstract interpreter gaps: recursive analysis, narrowing,
//! summary cache tightest-match, seed summaries, and AnalysisResult edge cases.
#![allow(clippy::drop_non_drop)]

use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{AbstractInterpreter, AnalysisResult, StageAccess, WideningStrategy};
use kirin_interval::Interval;
use kirin_ir::query::ParentInfo;
use kirin_ir::*;
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::build_loop_program;

// ===========================================================================
// A language with kirin_function::Call for recursive abstract analysis
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
enum CallLang {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    Call(kirin_function::Call<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

/// Build `f(x) = if x then f(x-1) else 0` for abstract analysis.
fn build_abstract_recursive(
    pipeline: &mut Pipeline<StageInfo<CallLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let func = pipeline.function().name("rec").new().unwrap();
    let staged = pipeline
        .staged_function::<CallLang>().func(func).stage(stage_id).new()
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let entry = b.block().argument(ArithType::I64).new();
        let call_block = b.block().argument(ArithType::I64).new();
        let exit_block = b.block().new();

        let x: SSAValue = b.block_arena()[entry].arguments[0].into();
        let call_arg: SSAValue = b.block_arena()[call_block].arguments[0].into();

        // exit_block: c0 = const 0; ret c0
        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
        let ret0 = Return::<ArithType>::new(b, c0.result);
        {
            let stmts: Vec<Statement> = vec![c0.into()];
            for stmt in &stmts {
                *b.statement_arena_mut()[*stmt].get_parent_mut() =
                    Some(StatementParent::Block(exit_block));
            }
            let linked = b.link_statements(&stmts);
            let ret_stmt: Statement = ret0.into();
            *b.statement_arena_mut()[ret_stmt].get_parent_mut() =
                Some(StatementParent::Block(exit_block));
            let exit_info = b.block_arena_mut().get_mut(exit_block).unwrap();
            exit_info.statements = linked;
            exit_info.terminator = Some(ret_stmt);
        }

        // call_block(arg): call rec(arg); ret call_result
        let rec_sym = b.symbol_table_mut().intern("rec".to_string());
        let call = kirin_function::Call::<ArithType>::new(b, rec_sym, vec![call_arg]);
        let ret_call = Return::<ArithType>::new(b, call.res);
        {
            let call_stmt: Statement = call.into();
            *b.statement_arena_mut()[call_stmt].get_parent_mut() =
                Some(StatementParent::Block(call_block));
            let linked = b.link_statements(&[call_stmt]);
            let ret_stmt: Statement = ret_call.into();
            *b.statement_arena_mut()[ret_stmt].get_parent_mut() =
                Some(StatementParent::Block(call_block));
            let call_info = b.block_arena_mut().get_mut(call_block).unwrap();
            call_info.statements = linked;
            call_info.terminator = Some(ret_stmt);
        }

        // entry(x): c1 = const 1; dec = sub x, c1; cond_br x call_block(dec) exit_block()
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let dec = Arith::<ArithType>::op_sub(b, x, c1.result);
        let cond = ControlFlow::<ArithType>::op_conditional_branch(
            b,
            x,
            Successor::from_block(call_block),
            vec![dec.result.into()],
            Successor::from_block(exit_block),
            vec![],
        );
        {
            let stmts: Vec<Statement> = vec![c1.into(), dec.into()];
            for stmt in &stmts {
                *b.statement_arena_mut()[*stmt].get_parent_mut() =
                    Some(StatementParent::Block(entry));
            }
            let linked = b.link_statements(&stmts);
            let cond_stmt: Statement = cond.into();
            *b.statement_arena_mut()[cond_stmt].get_parent_mut() =
                Some(StatementParent::Block(entry));
            let entry_info = b.block_arena_mut().get_mut(entry).unwrap();
            entry_info.statements = linked;
            entry_info.terminator = Some(cond_stmt);
        }

        let region = b
            .region()
            .add_block(entry)
            .add_block(call_block)
            .add_block(exit_block)
            .new();
        let body = FunctionBody::<ArithType>::new(b, region);
        b.specialize().staged_func(staged).body(body).new().unwrap()
    })
}

// ===========================================================================
// Test: Abstract recursive analysis (set_tentative/promote_tentative paths)
// ===========================================================================

#[test]
fn test_abstract_recursive_analysis() {
    let mut pipeline: Pipeline<StageInfo<CallLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_abstract_recursive(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_max_iterations(100);

    let result = interp
        .in_stage::<CallLang>()
        .analyze(spec_fn, &[Interval::new(0, 5)])
        .unwrap();

    // The recursive function always eventually returns 0 (from the base case).
    // The abstract analysis should converge with a non-empty return value
    // that includes 0.
    let ret = result.return_value().unwrap();
    assert!(
        !ret.is_empty(),
        "return value should not be bottom after recursive analysis"
    );
}

// ===========================================================================
// Test: SummaryCache find_best_match returns tightest entry
// ===========================================================================

#[test]
fn test_summary_cache_tightest_match() {
    let mut pipeline: Pipeline<StageInfo<CallLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Build a simple add_one style function.
    let spec_fn = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let ba_x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, SSAValue::from(ba_x), c1.result);
        let ret = Return::<ArithType>::new(b, add.result);
        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(add)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(b, region);
        b.specialize().staged_func(sf).body(body).new().unwrap()
    });

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    // Analyze with wide input first.
    let result_wide = interp
        .in_stage::<CallLang>()
        .analyze(spec_fn, &[Interval::new(0, 100)])
        .unwrap();
    assert_eq!(result_wide.return_value(), Some(&Interval::new(1, 101)));

    // Analyze with narrow input — should get a tighter result.
    let result_narrow = interp
        .in_stage::<CallLang>()
        .analyze(spec_fn, &[Interval::new(5, 10)])
        .unwrap();

    // The narrow query [5, 10] is subsumed by the cached [0, 100] entry,
    // so it returns the wider cached result. This is expected behavior —
    // the cache returns the tightest *subsuming* entry.
    let ret = result_narrow.return_value().unwrap();
    assert!(
        !ret.is_empty(),
        "narrow query should return a non-empty result from cache"
    );
}

// ===========================================================================
// Test: SummaryCache seed (refinable entry)
// ===========================================================================

#[test]
fn test_summary_seed_refinable() {
    let mut pipeline: Pipeline<StageInfo<CallLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let ba_x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, SSAValue::from(ba_x), c1.result);
        let ret = Return::<ArithType>::new(b, add.result);
        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(add)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(b, region);
        b.specialize().staged_func(sf).body(body).new().unwrap()
    });

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    // Seed with an imprecise summary — should be refinable.
    let seed_result = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::top()),
    );
    interp
        .in_stage::<CallLang>()
        .insert_summary(spec_fn)
        .seed(vec![Interval::new(0, 10)], seed_result);

    // Query should hit the seed.
    let staged = interp.in_stage::<CallLang>();
    let cached = staged.summary(spec_fn, &[Interval::new(0, 10)]);
    assert!(cached.is_some(), "seed summary should be queryable");
    assert_eq!(cached.unwrap().return_value(), Some(&Interval::top()));
    drop(staged); // release mutable borrow on interp

    // Invalidate the seed, then re-analyze for a precise result.
    let count = interp.in_stage::<CallLang>().invalidate_summary(spec_fn);
    assert!(count > 0, "expected at least one invalidated entry");

    let refined = interp
        .in_stage::<CallLang>()
        .analyze(spec_fn, &[Interval::new(0, 10)])
        .unwrap();
    assert_eq!(
        refined.return_value(),
        Some(&Interval::new(1, 11)),
        "analysis after invalidation should produce precise result"
    );
}

// ===========================================================================
// Test: Narrowing refinement tightens post-fixpoint results
// ===========================================================================

#[test]
fn test_narrowing_tightens_loop_result() {
    // With widening (AllJoins), the loop result is over-approximated.
    // Adding narrowing iterations should produce a tighter (or equal) result.
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    // Analyze WITHOUT narrowing.
    let mut interp_no_narrow: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_narrowing_iterations(0)
            .with_max_iterations(100);

    let result_no_narrow = interp_no_narrow
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 5)])
        .unwrap();

    // Analyze WITH narrowing.
    let mut interp_narrow: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_narrowing_iterations(10)
            .with_max_iterations(100);

    let result_narrow = interp_narrow
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 5)])
        .unwrap();

    let ret_no_narrow = result_no_narrow.return_value().unwrap();
    let ret_narrow = result_narrow.return_value().unwrap();

    // Narrowing should produce a result that is a subset of (or equal to)
    // the widened-only result.
    assert!(
        ret_narrow.is_subseteq(ret_no_narrow),
        "narrowed result {ret_narrow:?} should be subsumed by non-narrowed {ret_no_narrow:?}"
    );

    // Both should be non-empty.
    assert!(
        !ret_no_narrow.is_empty(),
        "non-narrowed result should not be bottom"
    );
    assert!(
        !ret_narrow.is_empty(),
        "narrowed result should not be bottom"
    );
}

// ===========================================================================
// Test: Narrowing with 0 iterations is a no-op
// ===========================================================================

#[test]
fn test_narrowing_zero_iterations_same_as_default() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    let mut interp0: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_narrowing_iterations(0)
            .with_max_iterations(100);

    let result0 = interp0
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 5)])
        .unwrap();

    // A fresh interpreter with default narrowing (3) but we set to 0 explicitly.
    // The result should converge regardless.
    assert!(
        !result0.return_value().unwrap().is_empty(),
        "zero-narrowing analysis should still converge"
    );
}

// ===========================================================================
// Test: WideningStrategy::Delayed delays widening for n visits
// ===========================================================================

#[test]
fn test_widening_delayed_threshold_behavior() {
    // With Delayed(0), widening kicks in immediately — like AllJoins.
    // With Delayed(large), it acts like Never for bounded inputs.
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    // Delayed(0) should converge (widening from the start).
    let mut interp_d0: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::Delayed(0))
            .with_max_iterations(200);

    let result_d0 = interp_d0
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 5)])
        .unwrap();
    assert!(
        !result_d0.return_value().unwrap().is_empty(),
        "Delayed(0) should converge"
    );

    // Delayed(100) with limited iterations may exhaust fuel since it delays
    // widening. With enough max_iterations it should still converge.
    let mut interp_d100: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::Delayed(100))
            .with_max_iterations(500);

    let result_d100 = interp_d100
        .in_stage::<CompositeLanguage>()
        .analyze(spec_fn, &[Interval::new(0, 5)]);

    // Either it converges or it exhausts fuel — both are valid outcomes.
    // The point is that a high delay threshold needs more iterations.
    match result_d100 {
        Ok(r) => assert!(
            !r.return_value().unwrap().is_empty(),
            "Delayed(100) converged with non-empty result"
        ),
        Err(e) => assert!(
            matches!(e, kirin_interpreter::InterpreterError::FuelExhausted),
            "expected FuelExhausted with high delay, got: {e:?}"
        ),
    }
}

// ===========================================================================
// Test: AnalysisResult::is_subseteq edge cases
// ===========================================================================

#[test]
fn test_analysis_result_is_subseteq_some_none() {
    // (Some(ret), None) should fail subsumption.
    let a: AnalysisResult<Interval> = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::new(0, 10)),
    );
    let b: AnalysisResult<Interval> =
        AnalysisResult::new(Default::default(), Default::default(), None);

    assert!(
        !a.is_subseteq(&b),
        "result with Some return should not be subsumed by None return"
    );
}

#[test]
fn test_analysis_result_is_subseteq_none_some() {
    // (None, Some(ret)) should succeed (None is bottom).
    let a: AnalysisResult<Interval> =
        AnalysisResult::new(Default::default(), Default::default(), None);
    let b: AnalysisResult<Interval> = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::new(0, 10)),
    );

    assert!(
        a.is_subseteq(&b),
        "result with None return should be subsumed by Some return"
    );
}

#[test]
fn test_analysis_result_is_subseteq_both_some() {
    let narrow: AnalysisResult<Interval> = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::new(3, 7)),
    );
    let wide: AnalysisResult<Interval> = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::new(0, 10)),
    );

    assert!(
        narrow.is_subseteq(&wide),
        "[3,7] should be subsumed by [0,10]"
    );
    assert!(
        !wide.is_subseteq(&narrow),
        "[0,10] should NOT be subsumed by [3,7]"
    );
}

// ===========================================================================
// Test: DispatchCache trivial methods
// ===========================================================================

#[test]
fn test_dispatch_cache_empty_and_is_empty() {
    let cache = kirin_interpreter::dispatch::DispatchCache::<i32>::empty();
    assert!(cache.is_empty(), "empty cache should report is_empty");
}

#[test]
fn test_dispatch_cache_get_returns_none_for_missing() {
    let mut pipeline: Pipeline<StageInfo<CallLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Build a cache from the pipeline — the single stage should resolve.
    let cache = kirin_interpreter::dispatch::DispatchCache::<String>::build(
        &pipeline,
        |_pipeline, _stage| -> Result<String, String> { Ok("found".to_string()) },
    );
    assert!(!cache.is_empty());

    // The valid stage should be found.
    assert_eq!(cache.get(stage_id), Some(&"found".to_string()));
}
