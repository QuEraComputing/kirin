//! Format-independent typed SSA/control-flow description for cross-version checks.
use std::collections::HashMap;

use super::BenchmarkLanguage;
use kirin::prelude::*;
use kirin_cf::ControlFlow;
use kirin_scf::StructuredControlFlow;
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
    let ty = |value: SSAValue| {
        assert_eq!(*value.expect_info(stage).ty(), kirin_arith::ArithType::I64);
        "int"
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
                BenchmarkLanguage::Scf(StructuredControlFlow::If(_)) => "if",
                BenchmarkLanguage::Scf(StructuredControlFlow::Yield(_)) => "yield",
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
