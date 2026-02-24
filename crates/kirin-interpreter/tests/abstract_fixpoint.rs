use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{AbstractInterpreter, WideningStrategy};
use kirin_interval::Interval;
use kirin_ir::{query::ParentInfo, *};
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::dump_function;

// ---------------------------------------------------------------------------
// IR builders
// ---------------------------------------------------------------------------

/// Build: entry(x): neg_x = neg x; cond_br x then=neg_block(neg_x) else=pos_block(x)
///        neg_block(val): ret val
///        pos_block(val): ret val
fn build_branch_fork_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry_block_node = stage.block().argument(ArithType::I64).new();

    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry_block_node.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // neg_block(val): receives neg_x via block arg, returns val
    let neg_block = stage.block().argument(ArithType::I64).new();
    let neg_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        neg_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_neg = ControlFlow::<ArithType>::op_return(stage, neg_val);
    {
        let neg_info: &mut Item<BlockInfo<CompositeLanguage>> =
            neg_block.get_info_mut(stage).unwrap();
        neg_info.terminator = Some(ret_neg.into());
    }

    // pos_block(val): receives x via block arg, returns val
    let pos_block = stage.block().argument(ArithType::I64).new();
    let pos_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        pos_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_pos = ControlFlow::<ArithType>::op_return(stage, pos_val);
    {
        let pos_info: &mut Item<BlockInfo<CompositeLanguage>> =
            pos_block.get_info_mut(stage).unwrap();
        pos_info.terminator = Some(ret_pos.into());
    }

    // Entry block: neg_x = neg x; cond_br x then=neg_block(neg_x) else=pos_block(x)
    let neg_result = kirin_arith::Arith::<ArithType>::op_neg(stage, x);

    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        x,
        Successor::from_block(neg_block),
        vec![neg_result.result.into()],
        Successor::from_block(pos_block),
        vec![x],
    );

    {
        let stmts: Vec<Statement> = vec![neg_result.into()];
        for &s in &stmts {
            let info = s.expect_info_mut(stage);
            *info.get_parent_mut() = Some(entry_block_node);
        }
        let linked = stage.link_statements(&stmts);

        let cond_br_stmt: Statement = cond_br.into();
        let info = cond_br_stmt.expect_info_mut(stage);
        *info.get_parent_mut() = Some(entry_block_node);

        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
            entry_block_node.get_info_mut(stage).unwrap();
        entry_info.statements = linked;
        entry_info.terminator = Some(cond_br_stmt);
    }

    let region = stage
        .region()
        .add_block(entry_block_node)
        .add_block(neg_block)
        .add_block(pos_block)
        .new();

    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

/// Build a loop where the loop variable flows via block arguments:
///   entry(x): br header(x)
///   header(i): cond_br i then=loop_body(i) else=loop_exit(i)
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
        let exit_info: &mut Item<BlockInfo<CompositeLanguage>> =
            loop_exit.get_info_mut(stage).unwrap();
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
    let br_back = ControlFlow::<ArithType>::op_branch(
        stage,
        Successor::from_block(header),
        vec![sum.result.into()],
    );
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

        let body_info: &mut Item<BlockInfo<CompositeLanguage>> =
            loop_body.get_info_mut(stage).unwrap();
        body_info.statements = linked;
        body_info.terminator = Some(br_stmt);
    }

    // entry: br header(x)
    let br_header =
        ControlFlow::<ArithType>::op_branch(stage, Successor::from_block(header), vec![x]);
    {
        let br_stmt: Statement = br_header.into();
        br_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(entry);
        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
            entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(br_stmt);
    }

    // header: cond_br i then=loop_body(i) else=loop_exit(i)
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        i,
        Successor::from_block(loop_body),
        vec![i],
        Successor::from_block(loop_exit),
        vec![i],
    );
    {
        let cond_stmt: Statement = cond_br.into();
        cond_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(header);
        let header_info: &mut Item<BlockInfo<CompositeLanguage>> =
            header.get_info_mut(stage).unwrap();
        header_info.terminator = Some(cond_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(header)
        .add_block(loop_body)
        .add_block(loop_exit)
        .new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

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
// Test 1: Straight-line constant propagation
// ---------------------------------------------------------------------------

/// Build `c1 = constant 10; c2 = constant 32; y = add c1, c2; return y`
/// Run through AbstractInterpreter and verify return value.
#[test]
fn test_abstract_interp_constants() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
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

    let result = interp.analyze::<CompositeLanguage>(spec_fn, &[]).unwrap();
    assert_eq!(result.return_value(), Some(&Interval::constant(42)));
}

// ---------------------------------------------------------------------------
// Test 2: Branching with Fork (undecidable condition)
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
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(-10, 10)])
        .unwrap();

    // The return value is the join of:
    // - neg path: neg([-10, 10]) = [-10, 10]
    // - pos path: [-10, 10]
    // join = [-10, 10]
    let ret = result.return_value().unwrap();
    assert_eq!(*ret, Interval::new(-10, 10));
}

// ---------------------------------------------------------------------------
// Test 3: Loop with back-edge, block args, and worklist convergence
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
        .analyze::<CompositeLanguage>(spec_fn, &[Interval::new(-5, 5)])
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
// Test 4: Summary caching via call()
// ---------------------------------------------------------------------------

/// Call the same function twice and verify the summary cache is populated
/// after the first call and reused on the second.
#[test]
fn test_abstract_interp_call_caches_summary() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
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
    let result1 = interp.analyze::<CompositeLanguage>(spec_fn, &[]).unwrap();
    assert_eq!(result1.return_value(), Some(&Interval::constant(10)));

    // Summary should be cached (args subsumed)
    assert!(interp.summary(spec_fn, &[]).is_some());
    assert_eq!(
        interp.summary(spec_fn, &[]).unwrap().return_value(),
        Some(&Interval::constant(10))
    );

    // Second call with same args — returns cached summary
    let result2 = interp.analyze::<CompositeLanguage>(spec_fn, &[]).unwrap();
    assert_eq!(result2.return_value(), Some(&Interval::constant(10)));
}
