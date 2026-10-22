//! Integration tests: classic (dense, per-point) liveness over real parsed
//! programs, with transfer dispatched through the dialects'
//! `Interpretable<I, ClassicLiveness>` rules. Semantics are deliberately
//! conventional: every use gens, purity-irrelevant, so dead code's operands DO
//! appear in the per-point sets. Filtering a set by strong liveness
//! (`kirin-demand`) drops values no root needs; that filter over-approximates
//! per-point strong liveness.

use kirin::prelude::{GetInfo, ParsePipelineText, Pipeline, SSAValue, StageInfo};
use kirin_arith::Arith;
use kirin_demand::{Demand, analyze_demand};
use kirin_interpreter::{Body, Callee, InterpreterError, ProgramPoint, Scoped};
use kirin_liveness::{DenseLiveness, LiveSet, analyze_dense};
use kirin_test_languages::ArithFunctionLanguage;

const PROGRAM: &str = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %cond: i64) {
    %dead = add %x, %x -> i64;
    cond_br %cond then=^then(%x) else=^else(%x);
  }
  ^then(%a: i64) {
    ret %a;
  }
  ^else(%b: i64) {
    %neg = neg %b -> i64;
    ret %neg;
  }
}
"#;

const ENGINE_REUSE_PROGRAM: &str = r#"
stage @test fn @first(i64) -> i64;
stage @test fn @second(i64) -> i64;

specialize @test fn @first(i64) -> i64 cfg {
  ^entry(%x: i64) {
    ret %x;
  }
}

specialize @test fn @second(i64) -> i64 cfg {
  ^entry(%y: i64) {
    %neg = neg %y -> i64;
    ret %neg;
  }
}
"#;

const DEAD_RESULT_PROGRAM: &str = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %s = add %a, %b -> i64;
    ret %a;
  }
}
"#;

fn parse(program: &str) -> Pipeline<StageInfo<ArithFunctionLanguage>> {
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
    pipeline
}

/// The caller stage and canonical callable root for `function_name`.
fn function_root(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    function_name: &str,
) -> (kirin::prelude::CompileStage, Callee) {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let function = pipeline
        .lookup_function_by_name(function_name)
        .unwrap_or_else(|| panic!("function @{function_name} exists"));
    (stage_id, Callee::Function(function))
}

/// The parameters of the `index`-th block of `cfg`, as SSA values.
fn block_params(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::CFG,
    index: usize,
) -> Vec<SSAValue> {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");
    let block = cfg.blocks(stage).nth(index).expect("block index in range");
    block
        .expect_info(stage)
        .arguments()
        .iter()
        .copied()
        .map(SSAValue::from)
        .collect()
}

/// The `index`-th block of `cfg`.
fn nth_block(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::CFG,
    index: usize,
) -> kirin_ir::Block {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");
    cfg.blocks(stage).nth(index).expect("block index in range")
}

/// The statement whose definition matches `select`.
fn find_stmt(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::CFG,
    select: impl Fn(&ArithFunctionLanguage) -> bool,
) -> kirin_ir::Statement {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");
    for block in cfg.blocks(stage) {
        for stmt in block.statements(stage) {
            if select(stmt.definition(stage)) {
                return stmt;
            }
        }
        if let Some(terminator) = block.terminator(stage)
            && select(terminator.definition(stage))
        {
            return terminator;
        }
    }
    panic!("no matching statement in cfg");
}

fn live_set(values: &[SSAValue]) -> LiveSet {
    values.iter().copied().collect()
}

#[test]
fn classic_liveness_boundary_sets() {
    let pipeline = parse(PROGRAM);
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result = analyze_dense(&pipeline, caller_stage, callee).expect("analysis succeeds");
    let scope = result.root_scope();
    let (stage, Body::CFG(cfg)) = scope else {
        panic!("@main resolves to a CFG body")
    };

    let entry_params = block_params(&pipeline, cfg, 0);
    let (x, cond) = (entry_params[0], entry_params[1]);
    let then_param = block_params(&pipeline, cfg, 1)[0];
    let else_param = block_params(&pipeline, cfg, 2)[0];
    let entry = nth_block(&pipeline, cfg, 0);
    let then_block = nth_block(&pipeline, cfg, 1);
    let else_block = nth_block(&pipeline, cfg, 2);
    let point = |item| Scoped::new(scope, item);

    // live_in(entry): %x (used by add and both edges) and %cond (branch use).
    assert_eq!(
        result.point_facts(point(ProgramPoint::BlockEntry(entry))),
        Some(&live_set(&[x, cond]))
    );
    assert_eq!(
        result.point_facts(Scoped::new(
            (stage, Body::Block(entry)),
            ProgramPoint::BlockEntry(entry),
        )),
        None,
        "the same point under another body scope is a different fact"
    );
    // live_out(entry): both successors' live-ins mapped across the edges —
    // {%a} → {%x}, {%b} → {%x}; the branch condition is a terminator *use*,
    // not part of the boundary set.
    assert_eq!(
        result.point_facts(point(ProgramPoint::BlockExit(entry))),
        Some(&live_set(&[x]))
    );
    assert_eq!(
        result.point_facts(point(ProgramPoint::BlockEntry(then_block))),
        Some(&live_set(&[then_param]))
    );
    assert_eq!(
        result.point_facts(point(ProgramPoint::BlockExit(then_block))),
        Some(&live_set(&[]))
    );
    assert_eq!(
        result.point_facts(point(ProgramPoint::BlockEntry(else_block))),
        Some(&live_set(&[else_param]))
    );
    assert_eq!(
        result.point_facts(point(ProgramPoint::BlockExit(else_block))),
        Some(&live_set(&[]))
    );
}

#[test]
fn reusing_dense_engine_replaces_the_previous_scoped_result() {
    let pipeline = parse(ENGINE_REUSE_PROGRAM);
    let (stage, first) = function_root(&pipeline, "first");
    let (_, second) = function_root(&pipeline, "second");
    let mut engine = DenseLiveness::<_, InterpreterError>::new(&pipeline);

    let first_scope = engine
        .analyze(stage, first)
        .expect("first callable analysis succeeds");
    let (_, Body::CFG(first_cfg)) = first_scope else {
        panic!("@first resolves to a CFG body")
    };
    let first_entry = nth_block(&pipeline, first_cfg, 0);
    assert!(
        engine
            .point_facts(Scoped::new(
                first_scope,
                ProgramPoint::BlockEntry(first_entry),
            ))
            .is_some()
    );

    let second_scope = engine
        .analyze(stage, second)
        .expect("second callable analysis succeeds");
    let (_, Body::CFG(second_cfg)) = second_scope else {
        panic!("@second resolves to a CFG body")
    };
    let second_entry = nth_block(&pipeline, second_cfg, 0);
    assert_eq!(
        engine.point_facts(Scoped::new(
            first_scope,
            ProgramPoint::BlockEntry(first_entry),
        )),
        None,
        "facts from the previous analysis are not retained"
    );
    assert!(
        engine
            .point_facts(Scoped::new(
                second_scope,
                ProgramPoint::BlockEntry(second_entry),
            ))
            .is_some()
    );
}

#[test]
fn classic_per_point_sets_gen_dead_uses() {
    let pipeline = parse(DEAD_RESULT_PROGRAM);
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result = analyze_dense(&pipeline, caller_stage, callee).expect("analysis succeeds");
    let scope = result.root_scope();
    let (_, Body::CFG(cfg)) = scope else {
        panic!("@main resolves to a CFG body")
    };

    let params = block_params(&pipeline, cfg, 0);
    let (a, b) = (params[0], params[1]);
    let add = find_stmt(&pipeline, cfg, |definition| {
        matches!(definition, ArithFunctionLanguage::Arith(Arith::Add { .. }))
    });
    // Classic semantics: the dead add still GENS its operands, so %b is live
    // before it — this is the conventional per-point meaning (the old strong
    // expectations were demand projections, not dense liveness).
    assert_eq!(
        result.point_facts(Scoped::new(scope, ProgramPoint::Before(add))),
        Some(&live_set(&[a, b]))
    );
    assert_eq!(
        result.point_facts(Scoped::new(scope, ProgramPoint::After(add))),
        Some(&live_set(&[a]))
    );
}

#[test]
fn classic_sets_filtered_by_demand_drop_unneeded_values() {
    let pipeline = parse(DEAD_RESULT_PROGRAM);
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let dense = analyze_dense(&pipeline, caller_stage, callee).expect("dense analysis succeeds");
    let demand = analyze_demand(&pipeline, caller_stage, callee, [Demand::Live])
        .expect("demand analysis succeeds");
    let scope = dense.root_scope();
    assert_eq!(scope, demand.root_scope());
    let (_, Body::CFG(cfg)) = scope else {
        panic!("@main resolves to a CFG body")
    };

    let params = block_params(&pipeline, cfg, 0);
    let (a, b) = (params[0], params[1]);
    let add = find_stmt(&pipeline, cfg, |definition| {
        matches!(definition, ArithFunctionLanguage::Arith(Arith::Add { .. }))
    });

    // Filtering the classic set by demand drops %b: it is classically live
    // before the dead add but not demanded.
    let classic = dense
        .point_facts(Scoped::new(scope, ProgramPoint::Before(add)))
        .expect("point reconstructed");
    let strong: LiveSet = classic
        .iter()
        .filter(|value| demand.is_demanded(*value))
        .collect();
    assert_eq!(strong, live_set(&[a]));
    assert!(!strong.contains(b));
}
