//! Format-independent typed SSA/control-flow description for cross-version checks.
use std::collections::HashMap;

use super::BenchmarkLanguage;
use kirin::prelude::*;
use kirin_cf::ControlFlow;
use serde_json::{Value, json};

type Stage = StageInfo<BenchmarkLanguage>;

fn statements(stage: &Stage, block: Block) -> Vec<Statement> {
    block
        .statements(stage)
        .chain(block.terminator(stage))
        .collect()
}

fn visit(stage: &Stage, block: Block, blocks: &mut Vec<Block>) {
    blocks.push(block);
    for stmt in statements(stage, block) {
        for child in stmt.expect_info(stage).definition().blocks() {
            visit(stage, *child, blocks);
        }
    }
}

pub fn describe(stage: &Stage) -> Value {
    let mut blocks = Vec::new();
    for stmt in stage.statement_arena().iter() {
        if let BenchmarkLanguage::Function { body, .. } = stmt.definition() {
            for block in body.blocks(stage) {
                visit(stage, block, &mut blocks);
            }
        }
    }
    let block_ids: HashMap<_, _> = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
    let mut values = HashMap::<SSAValue, usize>::new();
    for block in &blocks {
        for arg in &block.expect_info(stage).arguments {
            let next = values.len();
            values.insert((*arg).into(), next);
        }
        for stmt in statements(stage, *block) {
            for result in stmt.expect_info(stage).definition().results() {
                let next = values.len();
                values.insert((*result).into(), next);
            }
        }
    }
    let ty = |value: SSAValue| match value.expect_info(stage).ty() {
        kirin_test_languages::SimpleType::I64 => "int",
        kirin_test_languages::SimpleType::Bool => "bool",
        other => panic!("unexpected benchmark type: {other:?}"),
    };
    Value::Array(blocks.iter().map(|block| {
        let args: Vec<_> = block.expect_info(stage).arguments.iter().map(|v| {
            let v: SSAValue = (*v).into(); json!({"id": values[&v], "type": ty(v)})
        }).collect();
        let ops: Vec<_> = statements(stage, *block).into_iter().map(|stmt| {
            let definition = stmt.expect_info(stage).definition();
            let kind = match definition {
                BenchmarkLanguage::Arith(kirin_arith::Arith::Add { .. }) => "add",
                BenchmarkLanguage::Cf(ControlFlow::Branch { .. }) => "br",
                BenchmarkLanguage::Cf(ControlFlow::ConditionalBranch { .. }) => "cond_br",
                BenchmarkLanguage::Return(_) => "ret",
                _ => panic!("unexpected benchmark operation"),
            };
            let inputs: Vec<_> = definition.arguments().map(|v| values[v]).collect();
            let outputs: Vec<_> = definition.results().map(|v| {
                let v: SSAValue = (*v).into(); json!({"id": values[&v], "type": ty(v)})
            }).collect();
            let targets: Vec<_> = definition.successors().map(|b| block_ids[&b.target()]).collect();
            let bodies: Vec<_> = definition.blocks().map(|b| block_ids[b]).collect();
            json!({"kind": kind, "inputs": inputs, "outputs": outputs, "targets": targets, "bodies": bodies})
        }).collect();
        json!({"arguments": args, "ops": ops})
    }).collect())
}

/// Validate the supported benchmark IR subset, in addition to metadata consistency.
pub fn verify(stage: &Stage) {
    kirin::ir::verify_derived(stage).expect("consistent SSA/CFG metadata");
    let check_target = |target: Successor, args: &[SSAValue]| {
        let parameters = &target.expect_info(stage).arguments;
        assert_eq!(parameters.len(), args.len(), "branch argument arity");
        for (parameter, argument) in parameters.iter().zip(args) {
            let parameter: SSAValue = (*parameter).into();
            assert_eq!(
                parameter.expect_info(stage).ty(),
                argument.expect_info(stage).ty()
            );
        }
    };
    for item in stage.statement_arena().iter() {
        let require_int = |value: SSAValue| {
            assert_eq!(
                *value.expect_info(stage).ty(),
                kirin_test_languages::SimpleType::I64
            );
        };
        match item.definition() {
            BenchmarkLanguage::Arith(kirin_arith::Arith::Add { lhs, rhs, result }) => {
                require_int(*lhs);
                require_int(*rhs);
                require_int((*result).into());
            }
            BenchmarkLanguage::Return(value) => {
                for argument in value.arguments() {
                    require_int(*argument);
                }
            }
            _ => {}
        }
        if let BenchmarkLanguage::Function { body, signature } = item.definition() {
            let entry = body.blocks(stage).next().expect("function entry block");
            assert_eq!(
                entry.expect_info(stage).arguments.len(),
                signature.params().len()
            );
            for (argument, ty) in entry
                .expect_info(stage)
                .arguments
                .iter()
                .zip(signature.params())
            {
                let value: SSAValue = (*argument).into();
                assert_eq!(value.expect_info(stage).ty(), ty);
            }
            assert_eq!(*signature.ret(), kirin_test_languages::SimpleType::I64);
            for block in body.blocks(stage) {
                for statement in block.statements(stage) {
                    assert!(!statement.expect_info(stage).definition().is_terminator());
                }
            }
        }
    }
    for item in stage.block_arena().iter() {
        assert!(item.terminator.is_some(), "benchmark block must terminate");
        let definition = item.terminator.unwrap().expect_info(stage).definition();
        match definition {
            BenchmarkLanguage::Cf(ControlFlow::Branch { target, args }) => {
                check_target(*target, args)
            }
            BenchmarkLanguage::Cf(ControlFlow::ConditionalBranch {
                true_target,
                true_args,
                false_target,
                false_args,
                condition,
            }) => {
                assert_eq!(
                    *condition.expect_info(stage).ty(),
                    kirin_test_languages::SimpleType::Bool
                );
                check_target(*true_target, true_args);
                check_target(*false_target, false_args);
            }
            BenchmarkLanguage::Return(value) => assert_eq!(value.arguments().count(), 1),
            _ => panic!("unsupported or missing CFG terminator"),
        }
    }
    // All value types must be i64 or Bool; the description also resolves
    // every actual operand, result, and successor identifier.
    let _ = describe(stage);
}
