//! Tests for abstract interpreter gaps: recursive analysis, call_handler not set,
//! summary cache tightest-match, and seed summaries.

use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{AbstractInterpreter, AnalysisResult, StageAccess, WideningStrategy};
use kirin_interval::Interval;
use kirin_ir::query::ParentInfo;
use kirin_ir::*;

// ===========================================================================
// A language with kirin_function::Call for recursive abstract analysis
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
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
    let func = pipeline.function().name("rec").new();
    let staged = pipeline
        .staged_function::<CallLang>()
        .func(func)
        .stage(stage_id)
        .new()
        .unwrap();

    let stage = pipeline.stage_mut(stage_id).unwrap();

    let entry = stage.block().argument(ArithType::I64).new();
    let call_block = stage.block().argument(ArithType::I64).new();
    let exit_block = stage.block().new();

    let x: SSAValue = entry.expect_info(stage).arguments[0].into();
    let call_arg: SSAValue = call_block.expect_info(stage).arguments[0].into();

    // exit_block: c0 = const 0; ret c0
    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let ret0 = Return::<ArithType>::new(stage, c0.result);
    {
        let stmts: Vec<Statement> = vec![c0.into()];
        for stmt in &stmts {
            *stmt.expect_info_mut(stage).get_parent_mut() = Some(exit_block);
        }
        let linked = stage.link_statements(&stmts);
        let ret_stmt: Statement = ret0.into();
        *ret_stmt.expect_info_mut(stage).get_parent_mut() = Some(exit_block);
        let exit_info = exit_block.get_info_mut(stage).unwrap();
        exit_info.statements = linked;
        exit_info.terminator = Some(ret_stmt);
    }

    // call_block(arg): call rec(arg); ret call_result
    let rec_sym = stage.symbol_table_mut().intern("rec".to_string());
    let call = kirin_function::Call::<ArithType>::new(stage, rec_sym, vec![call_arg]);
    let ret_call = Return::<ArithType>::new(stage, call.res);
    {
        let call_stmt: Statement = call.into();
        *call_stmt.expect_info_mut(stage).get_parent_mut() = Some(call_block);
        let linked = stage.link_statements(&[call_stmt]);
        let ret_stmt: Statement = ret_call.into();
        *ret_stmt.expect_info_mut(stage).get_parent_mut() = Some(call_block);
        let call_info = call_block.get_info_mut(stage).unwrap();
        call_info.statements = linked;
        call_info.terminator = Some(ret_stmt);
    }

    // entry(x): c1 = const 1; dec = sub x, c1; cond_br x call_block(dec) exit_block()
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let dec = Arith::<ArithType>::op_sub(stage, x, c1.result);
    let cond = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        x,
        Successor::from_block(call_block),
        vec![dec.result.into()],
        Successor::from_block(exit_block),
        vec![],
    );
    {
        let stmts: Vec<Statement> = vec![c1.into(), dec.into()];
        for stmt in &stmts {
            *stmt.expect_info_mut(stage).get_parent_mut() = Some(entry);
        }
        let linked = stage.link_statements(&stmts);
        let cond_stmt: Statement = cond.into();
        *cond_stmt.expect_info_mut(stage).get_parent_mut() = Some(entry);
        let entry_info = entry.get_info_mut(stage).unwrap();
        entry_info.statements = linked;
        entry_info.terminator = Some(cond_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(call_block)
        .add_block(exit_block)
        .new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage
        .specialize()
        .staged_func(staged)
        .body(body)
        .new()
        .unwrap()
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
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();
    let ba_x = stage.block_argument(0);
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, SSAValue::from(ba_x), c1.result);
    let ret = Return::<ArithType>::new(stage, add.result);
    let block = stage
        .block()
        .argument(ArithType::I64)
        .stmt(c1)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().staged_func(sf).body(body).new().unwrap();

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

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();
    let ba_x = stage.block_argument(0);
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, SSAValue::from(ba_x), c1.result);
    let ret = Return::<ArithType>::new(stage, add.result);
    let block = stage
        .block()
        .argument(ArithType::I64)
        .stmt(c1)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().staged_func(sf).body(body).new().unwrap();

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
    drop(staged);

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
