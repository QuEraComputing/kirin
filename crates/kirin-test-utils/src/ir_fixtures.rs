//! Shared IR builder fixtures for interpreter integration tests.
//!
//! All builders construct programs using [`CompositeLanguage`].

use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_ir::{query::ParentInfo, *};
use kirin_test_languages::CompositeLanguage;

/// Build `c1 = constant 10; c2 = constant 32; y = add c1, c2; return y`.
pub fn build_constants(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(10));
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(32));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let ret = Return::<ArithType>::new(stage, add.result);

    let block = stage
        .block()
        .stmt(c1)
        .stmt(c2)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

/// Build `f(x) = c1 = const 1; sum = add(x, c1); ret sum`.
pub fn build_add_one(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
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
    stage.specialize().f(sf).body(body).new().unwrap()
}

/// Build `f() = c1 = const 5; c2 = const 10; sum = add(c1, c2); ret sum`.
///
/// Returns `(spec_fn, add_statement)` where `add_statement` can be used as a breakpoint.
pub fn build_linear_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> (SpecializedFunction, Statement) {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(5));
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(10));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let add_stmt: Statement = add.id;
    let ret = Return::<ArithType>::new(stage, add.result);

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

/// Build `select(x) = if x != 0 then x+1 else 42`.
///
/// ```text
/// entry(x):
///   c1 = const 1; sum = add x, c1; c42 = const 42
///   cond_br x then=truthy_block(sum) else=falsy_block(c42)
/// truthy_block(val): ret val
/// falsy_block(val): ret val
/// ```
pub fn build_select_program(
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

    // truthy_block(val): ret val
    let truthy_block = stage.block().argument(ArithType::I64).new();
    let truthy_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        truthy_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_truthy = Return::<ArithType>::new(stage, truthy_val);
    {
        let truthy_info: &mut Item<BlockInfo<CompositeLanguage>> =
            truthy_block.get_info_mut(stage).unwrap();
        truthy_info.terminator = Some(ret_truthy.into());
    }

    // falsy_block(val): ret val
    let falsy_block = stage.block().argument(ArithType::I64).new();
    let falsy_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        falsy_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_falsy = Return::<ArithType>::new(stage, falsy_val);
    {
        let falsy_info: &mut Item<BlockInfo<CompositeLanguage>> =
            falsy_block.get_info_mut(stage).unwrap();
        falsy_info.terminator = Some(ret_falsy.into());
    }

    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = kirin_arith::Arith::<ArithType>::op_add(stage, x, c1.result);
    let c42 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(42));

    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        x,
        Successor::from_block(truthy_block),
        vec![sum.result.into()],
        Successor::from_block(falsy_block),
        vec![c42.result.into()],
    );
    {
        let stmts: Vec<Statement> = vec![c1.into(), sum.into(), c42.into()];
        for &s in &stmts {
            let info = s.expect_info_mut(stage);
            *info.get_parent_mut() = Some(entry);
        }
        let linked = stage.link_statements(&stmts);

        let cond_br_stmt: Statement = cond_br.into();
        let info = cond_br_stmt.expect_info_mut(stage);
        *info.get_parent_mut() = Some(entry);

        let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
            entry.get_info_mut(stage).unwrap();
        entry_info.statements = linked;
        entry_info.terminator = Some(cond_br_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(truthy_block)
        .add_block(falsy_block)
        .new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

/// Build a branching program with neg/pos paths:
///
/// ```text
/// entry(x): neg_x = neg x; cond_br x then=neg_block(neg_x) else=pos_block(x)
/// neg_block(val): ret val
/// pos_block(val): ret val
/// ```
pub fn build_branch_fork_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry_block_node = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        entry_block_node.expect_info(si).arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // neg_block(val): ret val
    let neg_block = stage.block().argument(ArithType::I64).new();
    let neg_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        neg_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_neg = Return::<ArithType>::new(stage, neg_val);
    {
        let neg_info: &mut Item<BlockInfo<CompositeLanguage>> =
            neg_block.get_info_mut(stage).unwrap();
        neg_info.terminator = Some(ret_neg.into());
    }

    // pos_block(val): ret val
    let pos_block = stage.block().argument(ArithType::I64).new();
    let pos_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        pos_block.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_pos = Return::<ArithType>::new(stage, pos_val);
    {
        let pos_info: &mut Item<BlockInfo<CompositeLanguage>> =
            pos_block.get_info_mut(stage).unwrap();
        pos_info.terminator = Some(ret_pos.into());
    }

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
///
/// ```text
/// entry(x): br header(x)
/// header(i): cond_br i then=loop_body(i) else=loop_exit(i)
/// loop_body(val): c1 = const 1; sum = add val, c1; br header(sum)
/// loop_exit(result): ret result
/// ```
pub fn build_loop_program(
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
    let ret_exit = Return::<ArithType>::new(stage, exit_val);
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

/// Build an infinite loop (no increment in body):
///
/// ```text
/// entry(x): br header(x)
/// header(i): cond_br i body(i) exit(i)
/// body(val): br header(val)  (back-edge, passes val unchanged)
/// exit(result): ret result
/// ```
pub fn build_infinite_loop(
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

    // exit(result): ret result
    let exit = stage.block().argument(ArithType::I64).new();
    let exit_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        exit.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ret_exit = Return::<ArithType>::new(stage, exit_val);
    {
        let exit_info: &mut Item<BlockInfo<CompositeLanguage>> = exit.get_info_mut(stage).unwrap();
        exit_info.terminator = Some(ret_exit.into());
    }

    // body(val): br header(val)
    let body = stage.block().argument(ArithType::I64).new();
    let body_val: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        body.expect_info(si).arguments[0].into()
    };
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let br_back =
        ControlFlow::<ArithType>::op_branch(stage, Successor::from_block(header), vec![body_val]);
    {
        let br_stmt: Statement = br_back.into();
        br_stmt
            .expect_info_mut(stage)
            .get_parent_mut()
            .replace(body);
        let body_info: &mut Item<BlockInfo<CompositeLanguage>> = body.get_info_mut(stage).unwrap();
        body_info.terminator = Some(br_stmt);
    }

    // header: cond_br i body(i) exit(i)
    let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        i,
        Successor::from_block(body),
        vec![i],
        Successor::from_block(exit),
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

/// Build `f(x, y) = q = div x, y; ret q`.
pub fn build_div_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let ba_x = stage.block_argument(0);
    let ba_y = stage.block_argument(1);
    let div =
        kirin_arith::Arith::<ArithType>::op_div(stage, SSAValue::from(ba_x), SSAValue::from(ba_y));
    let ret = Return::<ArithType>::new(stage, div.result);

    let block = stage
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(div)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

/// Build `f(x, y) = r = rem x, y; ret r`.
pub fn build_rem_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let ba_x = stage.block_argument(0);
    let ba_y = stage.block_argument(1);
    let rem =
        kirin_arith::Arith::<ArithType>::op_rem(stage, SSAValue::from(ba_x), SSAValue::from(ba_y));
    let ret = Return::<ArithType>::new(stage, rem.result);

    let block = stage
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(rem)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

/// Resolve the first statement in a specialized function's entry block.
pub fn first_statement_of_specialization(
    pipeline: &Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
    spec_fn: SpecializedFunction,
) -> Option<Statement> {
    let stage_info = pipeline.stage(stage_id).unwrap();
    let spec_info = spec_fn.expect_info(stage_info);
    let body_stmt = *spec_info.body();
    let region = body_stmt
        .regions::<CompositeLanguage>(stage_info)
        .next()
        .unwrap();
    let entry = region.blocks(stage_info).next().unwrap();
    let block_info = entry.expect_info(stage_info);
    block_info.statements.head().copied()
}
