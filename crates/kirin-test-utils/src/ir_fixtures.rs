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
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(10));
        let c2 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(32));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, c1.result, c2.result);
        let ret = Return::<ArithType>::new(b, add.result);

        let block = b.block().stmt(c1).stmt(c2).stmt(add).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

/// Build `f(x) = c1 = const 1; sum = add(x, c1); ret sum`.
pub fn build_add_one(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
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
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

/// Build `f() = c1 = const 5; c2 = const 10; sum = add(c1, c2); ret sum`.
///
/// Returns `(spec_fn, add_statement)` where `add_statement` can be used as a breakpoint.
pub fn build_linear_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> (SpecializedFunction, Statement) {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(5));
        let c2 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(10));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, c1.result, c2.result);
        let add_stmt: Statement = add.id;
        let ret = Return::<ArithType>::new(b, add.result);

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
        (spec_fn, add_stmt)
    })
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
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let entry = b.block().argument(ArithType::I64).new();
        let x: SSAValue = b.block_arena()[entry].arguments[0].into();

        // truthy_block(val): ret val
        let truthy_block = b.block().argument(ArithType::I64).new();
        let truthy_val: SSAValue = b.block_arena()[truthy_block].arguments[0].into();
        let ret_truthy = Return::<ArithType>::new(b, truthy_val);
        {
            let truthy_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(truthy_block).unwrap();
            truthy_info.terminator = Some(ret_truthy.into());
        }

        // falsy_block(val): ret val
        let falsy_block = b.block().argument(ArithType::I64).new();
        let falsy_val: SSAValue = b.block_arena()[falsy_block].arguments[0].into();
        let ret_falsy = Return::<ArithType>::new(b, falsy_val);
        {
            let falsy_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(falsy_block).unwrap();
            falsy_info.terminator = Some(ret_falsy.into());
        }

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let sum = kirin_arith::Arith::<ArithType>::op_add(b, x, c1.result);
        let c42 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(42));

        let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
            b,
            x,
            Successor::from_block(truthy_block),
            vec![sum.result.into()],
            Successor::from_block(falsy_block),
            vec![c42.result.into()],
        );
        {
            let stmts: Vec<Statement> = vec![c1.into(), sum.into(), c42.into()];
            for &s in &stmts {
                let info = &mut b.statement_arena_mut()[s];
                *info.get_parent_mut() = Some(StatementParent::Block(entry));
            }
            let linked = b.link_statements(&stmts);

            let cond_br_stmt: Statement = cond_br.into();
            let info = &mut b.statement_arena_mut()[cond_br_stmt];
            *info.get_parent_mut() = Some(StatementParent::Block(entry));

            let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(entry).unwrap();
            entry_info.statements = linked;
            entry_info.terminator = Some(cond_br_stmt);
        }

        let region = b
            .region()
            .add_block(entry)
            .add_block(truthy_block)
            .add_block(falsy_block)
            .new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
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
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let entry_block_node = b.block().argument(ArithType::I64).new();
        let x: SSAValue = b.block_arena()[entry_block_node].arguments[0].into();

        // neg_block(val): ret val
        let neg_block = b.block().argument(ArithType::I64).new();
        let neg_val: SSAValue = b.block_arena()[neg_block].arguments[0].into();
        let ret_neg = Return::<ArithType>::new(b, neg_val);
        {
            let neg_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(neg_block).unwrap();
            neg_info.terminator = Some(ret_neg.into());
        }

        // pos_block(val): ret val
        let pos_block = b.block().argument(ArithType::I64).new();
        let pos_val: SSAValue = b.block_arena()[pos_block].arguments[0].into();
        let ret_pos = Return::<ArithType>::new(b, pos_val);
        {
            let pos_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(pos_block).unwrap();
            pos_info.terminator = Some(ret_pos.into());
        }

        let neg_result = kirin_arith::Arith::<ArithType>::op_neg(b, x);

        let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
            b,
            x,
            Successor::from_block(neg_block),
            vec![neg_result.result.into()],
            Successor::from_block(pos_block),
            vec![x],
        );
        {
            let stmts: Vec<Statement> = vec![neg_result.into()];
            for &s in &stmts {
                let info = &mut b.statement_arena_mut()[s];
                *info.get_parent_mut() = Some(StatementParent::Block(entry_block_node));
            }
            let linked = b.link_statements(&stmts);

            let cond_br_stmt: Statement = cond_br.into();
            let info = &mut b.statement_arena_mut()[cond_br_stmt];
            *info.get_parent_mut() = Some(StatementParent::Block(entry_block_node));

            let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(entry_block_node).unwrap();
            entry_info.statements = linked;
            entry_info.terminator = Some(cond_br_stmt);
        }

        let region = b
            .region()
            .add_block(entry_block_node)
            .add_block(neg_block)
            .add_block(pos_block)
            .new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
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
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let entry = b.block().argument(ArithType::I64).new();
        let x: SSAValue = b.block_arena()[entry].arguments[0].into();

        let header = b.block().argument(ArithType::I64).new();
        let i: SSAValue = b.block_arena()[header].arguments[0].into();

        // loop_exit(result): ret result
        let loop_exit = b.block().argument(ArithType::I64).new();
        let exit_val: SSAValue = b.block_arena()[loop_exit].arguments[0].into();
        let ret_exit = Return::<ArithType>::new(b, exit_val);
        {
            let exit_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(loop_exit).unwrap();
            exit_info.terminator = Some(ret_exit.into());
        }

        // loop_body(val): c1 = const 1; sum = add val, c1; br header(sum)
        let loop_body = b.block().argument(ArithType::I64).new();
        let body_val: SSAValue = b.block_arena()[loop_body].arguments[0].into();

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let sum = kirin_arith::Arith::<ArithType>::op_add(b, body_val, c1.result);
        let br_back = ControlFlow::<ArithType>::op_branch(
            b,
            Successor::from_block(header),
            vec![sum.result.into()],
        );
        {
            let stmts: Vec<Statement> = vec![c1.into(), sum.into()];
            for &s in &stmts {
                let info = &mut b.statement_arena_mut()[s];
                *info.get_parent_mut() = Some(StatementParent::Block(loop_body));
            }
            let linked = b.link_statements(&stmts);

            let br_stmt: Statement = br_back.into();
            b.statement_arena_mut()[br_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(loop_body));

            let body_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(loop_body).unwrap();
            body_info.statements = linked;
            body_info.terminator = Some(br_stmt);
        }

        // entry: br header(x)
        let br_header =
            ControlFlow::<ArithType>::op_branch(b, Successor::from_block(header), vec![x]);
        {
            let br_stmt: Statement = br_header.into();
            b.statement_arena_mut()[br_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(entry));
            let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(entry).unwrap();
            entry_info.terminator = Some(br_stmt);
        }

        // header: cond_br i then=loop_body(i) else=loop_exit(i)
        let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
            b,
            i,
            Successor::from_block(loop_body),
            vec![i],
            Successor::from_block(loop_exit),
            vec![i],
        );
        {
            let cond_stmt: Statement = cond_br.into();
            b.statement_arena_mut()[cond_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(header));
            let header_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(header).unwrap();
            header_info.terminator = Some(cond_stmt);
        }

        let region = b
            .region()
            .add_block(entry)
            .add_block(header)
            .add_block(loop_body)
            .add_block(loop_exit)
            .new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
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
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let entry = b.block().argument(ArithType::I64).new();
        let x: SSAValue = b.block_arena()[entry].arguments[0].into();

        let header = b.block().argument(ArithType::I64).new();
        let i: SSAValue = b.block_arena()[header].arguments[0].into();

        // exit(result): ret result
        let exit = b.block().argument(ArithType::I64).new();
        let exit_val: SSAValue = b.block_arena()[exit].arguments[0].into();
        let ret_exit = Return::<ArithType>::new(b, exit_val);
        {
            let exit_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(exit).unwrap();
            exit_info.terminator = Some(ret_exit.into());
        }

        // body(val): br header(val)
        let body = b.block().argument(ArithType::I64).new();
        let body_val: SSAValue = b.block_arena()[body].arguments[0].into();
        let br_back =
            ControlFlow::<ArithType>::op_branch(b, Successor::from_block(header), vec![body_val]);
        {
            let br_stmt: Statement = br_back.into();
            b.statement_arena_mut()[br_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(body));
            let body_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(body).unwrap();
            body_info.terminator = Some(br_stmt);
        }

        // header: cond_br i body(i) exit(i)
        let cond_br = ControlFlow::<ArithType>::op_conditional_branch(
            b,
            i,
            Successor::from_block(body),
            vec![i],
            Successor::from_block(exit),
            vec![i],
        );
        {
            let cond_stmt: Statement = cond_br.into();
            b.statement_arena_mut()[cond_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(header));
            let header_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(header).unwrap();
            header_info.terminator = Some(cond_stmt);
        }

        // entry: br header(x)
        let br_header =
            ControlFlow::<ArithType>::op_branch(b, Successor::from_block(header), vec![x]);
        {
            let br_stmt: Statement = br_header.into();
            b.statement_arena_mut()[br_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(entry));
            let entry_info: &mut Item<BlockInfo<CompositeLanguage>> =
                b.block_arena_mut().get_mut(entry).unwrap();
            entry_info.terminator = Some(br_stmt);
        }

        let region = b
            .region()
            .add_block(entry)
            .add_block(header)
            .add_block(body)
            .add_block(exit)
            .new();
        let func_body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize()
            .staged_func(sf)
            .body(func_body)
            .new()
            .unwrap()
    })
}

/// Build `f(x, y) = q = div x, y; ret q`.
pub fn build_div_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let ba_x = b.block_argument().index(0);
        let ba_y = b.block_argument().index(1);
        let div =
            kirin_arith::Arith::<ArithType>::op_div(b, SSAValue::from(ba_x), SSAValue::from(ba_y));
        let ret = Return::<ArithType>::new(b, div.result);

        let block = b
            .block()
            .argument(ArithType::I64)
            .argument(ArithType::I64)
            .stmt(div)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

/// Build `f(x, y) = r = rem x, y; ret r`.
pub fn build_rem_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let ba_x = b.block_argument().index(0);
        let ba_y = b.block_argument().index(1);
        let rem =
            kirin_arith::Arith::<ArithType>::op_rem(b, SSAValue::from(ba_x), SSAValue::from(ba_y));
        let ret = Return::<ArithType>::new(b, rem.result);

        let block = b
            .block()
            .argument(ArithType::I64)
            .argument(ArithType::I64)
            .stmt(rem)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
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
