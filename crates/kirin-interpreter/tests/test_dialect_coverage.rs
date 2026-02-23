//! Extended test coverage for interpreter features using CompositeLanguage.
//!
//! Covers: fuel exhaustion, breakpoints, sequential calls, abstract widening
//! strategies, fixed summaries, summary invalidation/GC, and AnalysisResult queries.

use std::collections::HashSet;

use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{
    AbstractInterpreter, AnalysisResult, ConcreteExt, Continuation, InterpreterError,
    StackInterpreter, WideningStrategy,
};
use kirin_ir::{query::ParentInfo, *};
use kirin_test_utils::Interval;
use kirin_test_utils::CompositeLanguage;

// ===========================================================================
// IR builder helpers
// ===========================================================================

/// Build an infinite loop with block args:
///   entry(x): br header(x)
///   header(i): cond_br i body(i) exit(i)
///   body(val): br header(val)  (back-edge, passes val unchanged)
///   exit(result): ret result
fn build_infinite_loop(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    // Create entry block first to get real block arg
    let entry = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        entry.expect_info(si).arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // header(i) — loop target with block arg
    let header = stage.block().argument(ArithType::I64).new();
    let i: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        header.expect_info(si).arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // exit(result): ret result
    let exit = stage.block().argument(ArithType::I64).new();
    let exit_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        exit.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_exit = ControlFlow::<ArithType>::op_return(stage, exit_val);
    {
        let exit_info: &mut Item<BlockInfo<CompositeLanguage>> = exit.get_info_mut(stage).unwrap();
        exit_info.terminator = Some(ret_exit.into());
    }

    // body(val): br header(val) (back-edge, passes val unchanged)
    let body = stage.block().argument(ArithType::I64).new();
    let body_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        body.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let br_back = ControlFlow::<ArithType>::op_branch(stage, header, vec![body_val]);
    {
        let br_stmt: Statement = br_back.into();
        br_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(body);
        let body_info: &mut Item<BlockInfo<CompositeLanguage>> = body.get_info_mut(stage).unwrap();
        body_info.terminator = Some(br_stmt);
    }

    // header terminator: cond_br i body(i) exit(i)
    let cond_br =
        ControlFlow::<ArithType>::op_conditional_branch(stage, i, body, vec![i], exit, vec![i]);
    {
        let cond_stmt: Statement = cond_br.into();
        cond_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(header);
        let header_info: &mut Item<BlockInfo<CompositeLanguage>> = header.get_info_mut(stage).unwrap();
        header_info.terminator = Some(cond_stmt);
    }

    // entry terminator: br header(x)
    let br_header = ControlFlow::<ArithType>::op_branch(stage, header, vec![x]);
    {
        let br_stmt: Statement = br_header.into();
        br_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(entry);
        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> = entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(br_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(header)
        .add_block(body)
        .add_block(exit)
        .new();
    let func_body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(func_body).new().unwrap()
}

/// Build `f() = c1 = const 5; c2 = const 10; sum = add(c1, c2); ret sum`
/// Returns (spec_fn, add_statement) where add_statement can be used as breakpoint.
fn build_linear_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> (SpecializedFunction, Statement) {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(5));
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(10));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let add_stmt: Statement = add.id;
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .stmt(c1)
        .stmt(c2)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let func_body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(func_body).new().unwrap();
    (spec_fn, add_stmt)
}

/// Build `f(x) = c1 = const 1; sum = add(x, c1); ret sum`
fn build_add_one(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let ba_x = stage.block_argument(0);
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, SSAValue::from(ba_x), c1.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .argument(ArithType::I64)
        .stmt(c1)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let func_body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(func_body).new().unwrap()
}

/// Build a loop with block args for the loop variable:
///   entry(x): br header(x)
///   header(i): cond_br i loop_body(i) loop_exit(i)
///   loop_body(val): c1 = const 1; sum = add val, c1; br header(sum)
///   loop_exit(result): ret result
fn build_loop_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        entry.expect_info(si).arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // header(i) — loop header with block arg
    let header = stage.block().argument(ArithType::I64).new();
    let i: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        header.expect_info(si).arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // loop_exit(result): ret result
    let loop_exit = stage.block().argument(ArithType::I64).new();
    let exit_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        loop_exit.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_exit = ControlFlow::<ArithType>::op_return(stage, exit_val);
    {
        let exit_info: &mut Item<BlockInfo<CompositeLanguage>> = loop_exit.get_info_mut(stage).unwrap();
        exit_info.terminator = Some(ret_exit.into());
    }

    // loop_body(val): c1 = const 1; sum = add val, c1; br header(sum)
    let loop_body = stage.block().argument(ArithType::I64).new();
    let body_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        loop_body.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = kirin_arith::Arith::<ArithType>::op_add(stage, body_val, c1.result);
    let br_back = ControlFlow::<ArithType>::op_branch(stage, header, vec![sum.result.into()]);
    {
        let stmts: Vec<Statement> = vec![c1.into(), sum.into()];
        for &s in &stmts {
            let info = s.expect_info_mut(stage);
            *info.get_parent_mut() = Some(loop_body);
        }
        let linked = stage.link_statements(&stmts);

        let br_stmt: Statement = br_back.into();
        br_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(loop_body);

        let body_info: &mut Item<BlockInfo<CompositeLanguage>> = loop_body.get_info_mut(stage).unwrap();
        body_info.statements = linked;
        body_info.terminator = Some(br_stmt);
    }

    // entry: br header(x)
    let br_header = ControlFlow::<ArithType>::op_branch(stage, header, vec![x]);
    {
        let br_stmt: Statement = br_header.into();
        br_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(entry);
        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> = entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(br_stmt);
    }

    // header: cond_br i loop_body(i) loop_exit(i)
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        i,
        loop_body,
        vec![i],
        loop_exit,
        vec![i],
    );
    {
        let cond_stmt: Statement = cond_br.into();
        cond_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(header);
        let header_info: &mut Item<BlockInfo<CompositeLanguage>> = header.get_info_mut(stage).unwrap();
        header_info.terminator = Some(cond_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(header)
        .add_block(loop_body)
        .add_block(loop_exit)
        .new();
    let func_body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(func_body).new().unwrap()
}

/// Build a multi-block branching program with block args:
///   entry(x): c1 = const 1; sum = add x, c1; c42 = const 42;
///             cond_br x then=then_block(sum) else=else_block(c42)
///   then_block(val): ret val
///   else_block(val): ret val
fn build_multi_block(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    // Create entry block first to get real block arg SSA
    let entry = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        entry.expect_info(si).arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // then_block(val): ret val
    let then_block = stage.block().argument(ArithType::I64).new();
    let then_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        then_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_then = ControlFlow::<ArithType>::op_return(stage, then_val);
    {
        let then_info: &mut Item<BlockInfo<CompositeLanguage>> = then_block.get_info_mut(stage).unwrap();
        then_info.terminator = Some(ret_then.into());
    }

    // else_block(val): ret val
    let else_block = stage.block().argument(ArithType::I64).new();
    let else_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        else_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_else = ControlFlow::<ArithType>::op_return(stage, else_val);
    {
        let else_info: &mut Item<BlockInfo<CompositeLanguage>> = else_block.get_info_mut(stage).unwrap();
        else_info.terminator = Some(ret_else.into());
    }

    // Entry block statements: c1 = const 1; sum = add x, c1; c42 = const 42
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, x, c1.result);
    let c42 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(42));

    // cond_br x then=then_block(sum) else=else_block(c42)
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        x,
        then_block,
        vec![add.result.into()],
        else_block,
        vec![c42.result.into()],
    );
    {
        let stmts: Vec<Statement> = vec![c1.into(), add.into(), c42.into()];
        for &s in &stmts {
            let info = s.expect_info_mut(stage);
            *info.get_parent_mut() = Some(entry);
        }
        let linked = stage.link_statements(&stmts);

        let cond_stmt: Statement = cond_br.into();
        cond_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(entry);

        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> = entry.get_info_mut(stage).unwrap();
        entry_info.statements = linked;
        entry_info.terminator = Some(cond_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(then_block)
        .add_block(else_block)
        .new();
    let func_body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(func_body).new().unwrap()
}

// ===========================================================================
// Concrete interpreter tests
// ===========================================================================

#[test]
fn test_concrete_fuel_exhaustion() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_infinite_loop(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_fuel(20);

    let result = interp.call::<CompositeLanguage>(spec_fn, &[42]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, InterpreterError::FuelExhausted),
        "expected FuelExhausted, got: {err:?}"
    );
}

#[test]
fn test_concrete_breakpoints() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let (spec_fn, add_stmt) = build_linear_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Resolve entry and push frame manually for run_until_break
    let stage_info = pipeline.stage(stage_id).unwrap();
    let spec_info = spec_fn.expect_info(stage_info);
    let body_stmt = *spec_info.body();
    let regions: Vec<_> = body_stmt.regions::<CompositeLanguage>(stage_info).collect();
    let blocks: Vec<_> = regions[0].blocks(stage_info).collect();
    let block_info = blocks[0].expect_info(stage_info);
    let first_stmt = block_info.statements.head().copied();

    let frame = kirin_interpreter::Frame::new(spec_fn, first_stmt);
    interp.push_call_frame(frame).unwrap();

    // Set breakpoint at the add statement
    interp.set_breakpoints(HashSet::from([add_stmt]));

    // Run until break — should stop before executing add
    let control = interp.run_until_break::<CompositeLanguage>().unwrap();
    assert!(
        matches!(control, Continuation::Ext(ConcreteExt::Break)),
        "expected Break, got: {control:?}"
    );

    // Clear breakpoints and continue to completion
    interp.clear_breakpoints();
    let control = interp.run::<CompositeLanguage>().unwrap();
    match control {
        Continuation::Return(v) => assert_eq!(v, 15, "expected 5 + 10 = 15"),
        other => panic!("expected Return, got: {other:?}"),
    }
}

#[test]
fn test_concrete_sequential_calls() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Call f(5) -> 6
    let result = interp.call::<CompositeLanguage>(spec_fn, &[5]).unwrap();
    assert_eq!(result, 6);

    // Call f(10) -> 11 — interpreter resets between calls
    let result = interp.call::<CompositeLanguage>(spec_fn, &[10]).unwrap();
    assert_eq!(result, 11);

    // Call f(-1) -> 0
    let result = interp.call::<CompositeLanguage>(spec_fn, &[-1]).unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_concrete_fuel_sufficient() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_add_one(&mut pipeline, stage_id);

    // Enough fuel for a short program
    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_fuel(100);

    let result = interp.call::<CompositeLanguage>(spec_fn, &[5]).unwrap();
    assert_eq!(result, 6);
}

// ===========================================================================
// Abstract interpreter tests
// ===========================================================================

#[test]
fn test_abstract_widening_never() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_loop_program(&mut pipeline, stage_id);

    // WideningStrategy::Never only joins (no widening). With block args the
    // loop variable grows each iteration (add 1), so the ascending chain
    // never converges — expect FuelExhausted.
    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::Never)
            .with_max_iterations(50);

    let result = interp.analyze::<CompositeLanguage>(spec_fn, &[Interval::new(-5, 5)]);

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

    // Delayed(3): join for first 3 visits, then widen
    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::Delayed(3))
            .with_max_iterations(100);

    let result = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(-5, 5)])
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

    // AllJoins: widen at every join point — converges quickly
    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id)
            .with_widening(WideningStrategy::AllJoins)
            .with_max_iterations(100);

    let result = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    let ret = result.return_value().unwrap();
    assert!(
        !ret.is_empty(),
        "return value should not be bottom with AllJoins widening"
    );
}

#[test]
fn test_abstract_fixed_summary() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    // Insert a fixed summary that returns [100, 200] regardless of input
    let fixed_result = AnalysisResult::new(
        Default::default(),
        Default::default(),
        Some(Interval::new(100, 200)),
    );
    interp.insert_summary(spec_fn).fixed(fixed_result);

    // Analyze should return the fixed summary without computing
    let result = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(0, 10)])
        .unwrap();
    assert_eq!(result.return_value(), Some(&Interval::new(100, 200)));

    // Even with different args, the fixed summary is returned
    let result2 = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::top()])
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

    // Analyze to populate the cache
    let result = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(0, 10)])
        .unwrap();
    assert_eq!(result.return_value(), Some(&Interval::new(1, 11)));

    // Verify summary is cached
    assert!(interp.summary(spec_fn, &[Interval::new(0, 10)]).is_some());

    // Invalidate computed summaries
    let count = interp.invalidate_summary(spec_fn);
    assert!(count > 0, "expected at least one invalidated entry");

    // After invalidation, summary lookup should skip invalidated entries
    assert!(interp.summary(spec_fn, &[Interval::new(0, 10)]).is_none());

    // GC removes invalidated entries
    interp.gc_summaries();

    // Re-analyze should work fresh
    let result = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(0, 10)])
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

    // Analyze to populate cache
    interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::constant(5)])
        .unwrap();
    assert!(interp.summary(spec_fn, &[Interval::constant(5)]).is_some());

    // Remove all summaries unconditionally
    assert!(interp.remove_summary(spec_fn));

    // Now it should be gone
    assert!(interp.summary(spec_fn, &[Interval::constant(5)]).is_none());

    // Removing again returns false
    assert!(!interp.remove_summary(spec_fn));
}

#[test]
fn test_abstract_analysis_result_queries() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_multi_block(&mut pipeline, stage_id);

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    // Analyze with interval spanning zero -> fork at cond_br
    let result = interp
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    // Should have visited multiple blocks (entry + then + else = 3)
    let visited: Vec<_> = result.visited_blocks().collect();
    assert!(
        visited.len() >= 3,
        "expected at least 3 visited blocks, got {}",
        visited.len()
    );

    // Return value should be the join of both paths
    let ret = result.return_value().unwrap();
    assert!(!ret.is_empty(), "return value should not be bottom");
}

#[test]
fn test_abstract_analysis_result_ssa_values() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    // Build: c1 = const 7; c2 = const 3; sum = add c1, c2; ret sum
    let sf = stage.staged_function().new().unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(7));
    let c1_result = c1.result;
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(3));
    let c2_result = c2.result;
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let add_result = add.result;
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .stmt(c1)
        .stmt(c2)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let func_body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(func_body).new().unwrap();

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp.analyze::<CompositeLanguage>(spec_fn, &[]).unwrap();

    // Query individual SSA values
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
