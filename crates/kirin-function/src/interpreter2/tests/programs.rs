use kirin::prelude::query::ParentInfo;
use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;

use crate::{Call, FunctionBody, Return};

use super::harness::TestLanguage;

type TestPipeline = Pipeline<StageInfo<TestLanguage>>;

fn pipeline_with_stage() -> (TestPipeline, CompileStage) {
    let mut pipeline = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    (pipeline, stage_id)
}

fn define_named_function(
    pipeline: &mut TestPipeline,
    stage_id: CompileStage,
    name: &str,
    body: Statement,
    signature: Signature<ArithType>,
) -> SpecializedFunction {
    let (_, _, spec) = pipeline
        .define_function::<TestLanguage>()
        .name(name.to_string())
        .stage(stage_id)
        .signature(signature)
        .body(body)
        .new()
        .unwrap();
    spec
}

pub fn build_direct_call_program() -> (TestPipeline, CompileStage, SpecializedFunction) {
    let (mut pipeline, stage_id) = pipeline_with_stage();

    let callee_body = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let sum = Arith::<ArithType>::op_add(b, x, c1.result);
        let ret = Return::<ArithType>::new(b, vec![sum.result.into()]);
        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(sum)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        )
        .into()
    });
    define_named_function(
        &mut pipeline,
        stage_id,
        "add_one",
        callee_body,
        Signature::new(vec![ArithType::I64], ArithType::I64, ()),
    );

    let caller_body = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let add_one = b.symbol_table_mut().intern("add_one".to_string());
        let c40 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(40));
        let call = Call::<ArithType>::new(b, 1, add_one, vec![c40.result.into()]);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let sum = Arith::<ArithType>::op_add(b, call.results[0], c1.result);
        let ret = Return::<ArithType>::new(b, vec![sum.result.into()]);
        let block = b
            .block()
            .stmt(c40)
            .stmt(call)
            .stmt(c1)
            .stmt(sum)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        FunctionBody::<ArithType>::new(b, region, Signature::new(vec![], ArithType::I64, ())).into()
    });
    let caller = define_named_function(
        &mut pipeline,
        stage_id,
        "main",
        caller_body,
        Signature::new(vec![], ArithType::I64, ()),
    );

    (pipeline, stage_id, caller)
}

pub fn build_recursive_counter_program() -> (TestPipeline, CompileStage, SpecializedFunction) {
    let (mut pipeline, stage_id) = pipeline_with_stage();

    let body = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let count = b.symbol_table_mut().intern("count".to_string());

        let entry = b.block().argument(ArithType::I64).new();
        let n = b.block_arena()[entry].arguments[0].into();

        let recurse = b.block().argument(ArithType::I64).new();
        let recurse_n: SSAValue = b.block_arena()[recurse].arguments[0].into();

        let base = b.block().new();

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let dec = Arith::<ArithType>::op_sub(b, recurse_n, c1.result);
        let call = Call::<ArithType>::new(b, 1, count, vec![dec.result.into()]);
        let inc = Arith::<ArithType>::op_add(b, call.results[0], c1.result);
        let recurse_ret = Return::<ArithType>::new(b, vec![inc.result.into()]);
        {
            let stmts = vec![c1.into(), dec.into(), call.into(), inc.into()];
            for &stmt in &stmts {
                b.statement_arena_mut()[stmt]
                    .get_parent_mut()
                    .replace(StatementParent::Block(recurse));
            }
            let linked = b.link_statements(&stmts);
            let ret_stmt: Statement = recurse_ret.into();
            b.statement_arena_mut()[ret_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(recurse));
            let recurse_info = &mut b.block_arena_mut()[recurse];
            recurse_info.statements = linked;
            recurse_info.terminator = Some(ret_stmt);
        }

        let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
        let base_ret = Return::<ArithType>::new(b, vec![c0.result.into()]);
        {
            let stmts = vec![c0.into()];
            for &stmt in &stmts {
                b.statement_arena_mut()[stmt]
                    .get_parent_mut()
                    .replace(StatementParent::Block(base));
            }
            let linked = b.link_statements(&stmts);
            let ret_stmt: Statement = base_ret.into();
            b.statement_arena_mut()[ret_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(base));
            let base_info = &mut b.block_arena_mut()[base];
            base_info.statements = linked;
            base_info.terminator = Some(ret_stmt);
        }

        let cond = kirin_cf::ControlFlow::<ArithType>::op_conditional_branch(
            b,
            n,
            Successor::from_block(recurse),
            vec![n],
            Successor::from_block(base),
            vec![],
        );
        {
            let cond_stmt: Statement = cond.into();
            b.statement_arena_mut()[cond_stmt]
                .get_parent_mut()
                .replace(StatementParent::Block(entry));
            b.block_arena_mut()[entry].terminator = Some(cond_stmt);
        }

        let region = b
            .region()
            .add_block(entry)
            .add_block(recurse)
            .add_block(base)
            .new();
        FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        )
        .into()
    });
    let spec = define_named_function(
        &mut pipeline,
        stage_id,
        "count",
        body,
        Signature::new(vec![ArithType::I64], ArithType::I64, ()),
    );

    (pipeline, stage_id, spec)
}

pub fn build_multi_result_programs() -> (
    TestPipeline,
    CompileStage,
    SpecializedFunction,
    SpecializedFunction,
) {
    let (mut pipeline, stage_id) = pipeline_with_stage();

    let pair_body = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let dec = Arith::<ArithType>::op_sub(b, x, c1.result);
        let ret = Return::<ArithType>::new(b, vec![x, dec.result.into()]);
        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(dec)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        )
        .into()
    });
    let pair = define_named_function(
        &mut pipeline,
        stage_id,
        "pair",
        pair_body,
        Signature::new(vec![ArithType::I64], ArithType::I64, ()),
    );

    let caller_body = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let pair_sym = b.symbol_table_mut().intern("pair".to_string());
        let x = b.block_argument().index(0);
        let call = Call::<ArithType>::new(b, 2, pair_sym, vec![x]);
        let sum = Arith::<ArithType>::op_add(b, call.results[0], call.results[1]);
        let ret = Return::<ArithType>::new(b, vec![sum.result.into()]);
        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(call)
            .stmt(sum)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        )
        .into()
    });
    let caller = define_named_function(
        &mut pipeline,
        stage_id,
        "sum_pair",
        caller_body,
        Signature::new(vec![ArithType::I64], ArithType::I64, ()),
    );

    (pipeline, stage_id, pair, caller)
}
