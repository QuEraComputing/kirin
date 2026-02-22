use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::AbstractInterpreter;
use kirin_ir::*;
use kirin_test_utils::Interval;
use kirin_test_utils::TestDialect;

// ---------------------------------------------------------------------------
// Test 1: Straight-line constant propagation
// ---------------------------------------------------------------------------

/// Build `c1 = constant 10; c2 = constant 32; y = add c1, c2; return y`
/// Run through AbstractInterpreter and verify return value.
#[test]
fn test_abstract_interp_constants() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(10));
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(32));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .stmt(c1)
        .stmt(c2)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(body).new().unwrap();

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp.analyze::<TestDialect>(spec_fn, &[]).unwrap();
    assert_eq!(result.return_value(), Some(&Interval::constant(42)));
}

// ---------------------------------------------------------------------------
// Test 2: Branching with Fork (undecidable condition)
// ---------------------------------------------------------------------------

/// Build: if (is_truthy x) then -x else x
/// Pass Interval(-10, 10) which spans zero — is_truthy returns None → Fork.
/// Verify both branches explored, return value is join of both paths.
#[test]
fn test_abstract_interp_branch_fork() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Phase 1: Build entry block with argument (no terminator yet).
    // Must be built first so the real block argument SSA exists before
    // other blocks reference it.
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry_block_node = stage.block().argument(ArithType::I64).new();

    // Phase 2: Get the real block argument SSA value
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry_block_node.expect_info(si);
        bi.arguments[0].into()
    };

    // Phase 3: Build the branch targets using the real block arg
    let stage = pipeline.stage_mut(stage_id).unwrap();

    // neg_block: negate x and return
    let neg_result = kirin_arith::Arith::<ArithType>::op_neg(stage, x);
    let ret_neg = ControlFlow::<ArithType>::op_return(stage, neg_result.result);
    let neg_block = stage.block().stmt(neg_result).terminator(ret_neg).new();

    // non_neg_block: return x directly
    let ret_pos = ControlFlow::<ArithType>::op_return(stage, x);
    let non_neg_block = stage.block().terminator(ret_pos).new();

    // Phase 4: Add terminator to entry block
    let cond_br =
        ControlFlow::<ArithType>::op_conditional_branch(stage, x, neg_block, non_neg_block);
    {
        let entry_info: &mut Item<BlockInfo<TestDialect>> =
            entry_block_node.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(cond_br.into());
    }

    // Phase 5: Assemble region and function body
    let region = stage
        .region()
        .add_block(entry_block_node)
        .add_block(neg_block)
        .add_block(non_neg_block)
        .new();

    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(body).new().unwrap();

    // Run with interval spanning zero: both branches should be explored via Fork
    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    let result = interp
        .analyze::<TestDialect>(spec_fn, &[Interval::new(-10, 10)])
        .unwrap();

    // The return value is the join of:
    // - neg path: neg([-10, 10]) = [-10, 10]
    // - non-neg path: [-10, 10]
    // join = [-10, 10]
    let ret = result.return_value().unwrap();
    assert_eq!(*ret, Interval::new(-10, 10));
}

// ---------------------------------------------------------------------------
// Test 3: Loop with back-edge and worklist convergence
// ---------------------------------------------------------------------------

/// Build a loop with an unknown function input, exercising the worklist,
/// back-edge state propagation, and join stabilization:
///
///   entry(x):               -- function arg x = [-5, 5] (unknown input)
///     br header             -- jump to loop header (no block args)
///   header:                 -- no block args; x is in scope from entry
///     cond_br x loop_body loop_exit
///   loop_body:
///     c1 = const 1
///     sum = add x, c1
///     br header             -- back-edge (no block args)
///   loop_exit:
///     ret x
///
/// Since x = [-5, 5] spans zero, is_truthy returns None and
/// cond_br always returns Fork, exploring both paths. The back-edge
/// triggers worklist re-processing of header. The worklist converges
/// once no new SSA values appear.
///
/// This tests that:
/// - Unknown function input propagates through the loop
/// - The worklist converges (doesn't exceed max_iterations)
/// - Back-edge state propagation works correctly
/// - Joining at the loop header stabilizes
#[test]
fn test_abstract_interp_loop_convergence() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Phase 1: Build entry block with function argument (no terminator yet)
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry = stage.block().argument(ArithType::I64).new();

    // Phase 2: Get the real block argument SSA value (the unknown function input)
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry.expect_info(si);
        bi.arguments[0].into()
    };

    // Phase 3: Build header block (no block args — x is in scope from entry)
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let header = stage.block().new();

    // Phase 4: Build loop_body and loop_exit using x
    // loop_exit: ret x
    let ret_x = ControlFlow::<ArithType>::op_return(stage, x);
    let loop_exit = stage.block().terminator(ret_x).new();

    // loop_body: c1 = const 1; sum = add x, c1; br header
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = kirin_arith::Arith::<ArithType>::op_add(stage, x, c1.result);
    let br_back = ControlFlow::<ArithType>::op_branch(stage, header);
    let loop_body = stage.block().stmt(c1).stmt(sum).terminator(br_back).new();

    // Phase 5: Add terminators
    // entry: br header
    let br_header = ControlFlow::<ArithType>::op_branch(stage, header);
    {
        let entry_info: &mut Item<BlockInfo<TestDialect>> = entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(br_header.into());
    }

    // header: cond_br x loop_body loop_exit
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(stage, x, loop_body, loop_exit);
    {
        let header_info: &mut Item<BlockInfo<TestDialect>> = header.get_info_mut(stage).unwrap();
        header_info.terminator = Some(cond_br.into());
    }

    // Phase 6: Assemble region and function body
    let region = stage
        .region()
        .add_block(entry)
        .add_block(header)
        .add_block(loop_body)
        .add_block(loop_exit)
        .new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(body).new().unwrap();

    // Phase 7: Run abstract interpretation with Interval(-5, 5)
    // The unknown function input x spans zero → cond_br always Forks.
    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id).with_max_iterations(100);

    let result = interp
        .analyze::<TestDialect>(spec_fn, &[Interval::new(-5, 5)])
        .unwrap();

    // The analysis should converge and produce a return value from loop_exit.
    let ret = result.return_value().unwrap();

    // The return value is x at the header, which is the original function
    // input [-5, 5]. Branch/Fork don't carry block args, so x is never
    // updated — but the unknown input flows through the entire loop.
    assert_eq!(*ret, Interval::new(-5, 5));
}

// ---------------------------------------------------------------------------
// Test 4: Summary caching via call()
// ---------------------------------------------------------------------------

/// Call the same function twice and verify the summary cache is populated
/// after the first call and reused on the second.
#[test]
fn test_abstract_interp_call_caches_summary() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(7));
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(3));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .stmt(c1)
        .stmt(c2)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(body).new().unwrap();

    let mut interp: AbstractInterpreter<Interval, _> =
        AbstractInterpreter::new(&pipeline, stage_id);

    // First call — runs the analysis
    let result1 = interp.analyze::<TestDialect>(spec_fn, &[]).unwrap();
    assert_eq!(result1.return_value(), Some(&Interval::constant(10)));

    // Summary should be cached (args subsumed)
    assert!(interp.summary(spec_fn, &[]).is_some());
    assert_eq!(
        interp.summary(spec_fn, &[]).unwrap().return_value(),
        Some(&Interval::constant(10))
    );

    // Second call with same args — returns cached summary
    let result2 = interp.analyze::<TestDialect>(spec_fn, &[]).unwrap();
    assert_eq!(result2.return_value(), Some(&Interval::constant(10)));
}
