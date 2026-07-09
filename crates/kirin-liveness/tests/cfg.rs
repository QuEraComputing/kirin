//! Integration tests: strong liveness (sparse backward demand) over real
//! parsed programs, with transfer dispatched through the dialects'
//! `Interpretable<I, StrongDemand>` rules.

use kirin::prelude::{GetInfo, ParsePipelineText, Pipeline, SSAValue, StageInfo};
use kirin_arith::Arith;
use kirin_liveness::analyze_demand;
use kirin_test_languages::ArithFunctionLanguage;

const PROGRAM: &str = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
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

const DEAD_EDGE_ARG_PROGRAM: &str = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%live: i64, %dead: i64, %cond: i64) {
    cond_br %cond then=^then(%live) else=^else(%dead);
  }
  ^then(%a: i64) {
    ret %a;
  }
  ^else(%b: i64) {
    ret %live;
  }
}
"#;

fn parse(program: &str) -> Pipeline<StageInfo<ArithFunctionLanguage>> {
    let mut pipeline: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
    pipeline
}

/// The finalized stage id and the body cfg of `@main`.
fn main_cfg(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
) -> (kirin::prelude::CompileStage, kirin_ir::Cfg) {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");

    let sf = pipeline
        .resolve_staged_function("main", stage_id)
        .expect("@main is staged at @test");
    let sf_info = sf.get_info(stage).expect("staged function info");
    let spec = &sf_info.specializations()[0];
    let body = *spec.body();

    let cfg = match body.definition(stage) {
        ArithFunctionLanguage::Function { body, .. } => *body,
        other => panic!("expected a function body, got {other:?}"),
    };
    (stage_id, cfg)
}

/// The parameters of the `index`-th block of `cfg`, as SSA values.
fn block_params(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::Cfg,
    index: usize,
) -> Vec<SSAValue> {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");
    let block = cfg.blocks(stage).nth(index).expect("block index in range");
    block
        .expect_info(stage)
        .arguments
        .iter()
        .copied()
        .map(SSAValue::from)
        .collect()
}

/// Find an `Arith` statement in `cfg` matching `select`, returning what it
/// selects (e.g. the result and operands of the one `add`).
fn find_arith<R>(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::Cfg,
    select: impl Fn(&Arith<kirin_arith::ArithType>) -> Option<R>,
) -> R {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");
    for block in cfg.blocks(stage) {
        for stmt in block.statements(stage) {
            if let ArithFunctionLanguage::Arith(op) = stmt.definition(stage)
                && let Some(selected) = select(op)
            {
                return selected;
            }
        }
    }
    panic!("no matching arith statement in cfg");
}

#[test]
fn strong_liveness_over_branching_function() {
    let pipeline = parse(PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

    let entry_params = block_params(&pipeline, cfg, 0);
    let (x, cond) = (entry_params[0], entry_params[1]);
    let then_param = block_params(&pipeline, cfg, 1)[0];
    let else_param = block_params(&pipeline, cfg, 2)[0];

    // The dead `add` result is never demanded — its rule is purity-aware, so
    // it contributes no operand demand of its own.
    let (dead, add_lhs) = find_arith(&pipeline, cfg, |op| match op {
        Arith::Add { lhs, result, .. } => Some((SSAValue::from(*result), *lhs)),
        _ => None,
    });
    assert!(!result.is_demanded(dead));
    assert_eq!(add_lhs, x, "the add reads the entry parameter");

    // Roots and their transitive demands: return operands, the branch
    // condition, and the edge arguments feeding demanded successor params.
    assert!(result.is_demanded(cond), "branch condition is a root");
    assert!(
        result.is_demanded(then_param),
        "ret %a demands ^then's param"
    );
    assert!(result.is_demanded(else_param), "neg feeds ret in ^else");
    assert!(
        result.is_demanded(x),
        "both edges pass %x into demanded params"
    );

    let neg = find_arith(&pipeline, cfg, |op| match op {
        Arith::Neg { result, .. } => Some(SSAValue::from(*result)),
        _ => None,
    });
    assert!(result.is_demanded(neg), "ret %neg demands the neg result");
}

#[test]
fn unused_successor_block_argument_does_not_keep_edge_arg_live() {
    let pipeline = parse(DEAD_EDGE_ARG_PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

    let entry_params = block_params(&pipeline, cfg, 0);
    let (live, dead, cond) = (entry_params[0], entry_params[1], entry_params[2]);
    let then_param = block_params(&pipeline, cfg, 1)[0];
    let else_param = block_params(&pipeline, cfg, 2)[0];

    // `^else` returns `%live` directly (a dominated cross-block use), never
    // touching its own parameter — so the `%dead` edge argument stays dead.
    assert!(result.is_demanded(live));
    assert!(result.is_demanded(cond));
    assert!(result.is_demanded(then_param));
    assert!(!result.is_demanded(else_param), "^else's param is unused");
    assert!(
        !result.is_demanded(dead),
        "an edge arg feeding an undemanded param stays dead"
    );
}

// ===========================================================================
// Single-block transfer scenarios (ported from the former structural unit
// tests, now exercised end-to-end through parsed programs and dialect rules).
// ===========================================================================

const RET_PARAM_PROGRAM: &str = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    ret %x;
  }
}
"#;

const DEMANDED_RESULT_PROGRAM: &str = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %s = add %a, %b -> i64;
    ret %s;
  }
}
"#;

const DEAD_RESULT_PROGRAM: &str = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %s = add %a, %b -> i64;
    ret %a;
  }
}
"#;

#[test]
fn terminator_operands_become_demanded() {
    let pipeline = parse(RET_PARAM_PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

    let x = block_params(&pipeline, cfg, 0)[0];
    assert!(result.is_demanded(x));
}

#[test]
fn demanded_result_marks_operands_demanded() {
    let pipeline = parse(DEMANDED_RESULT_PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

    let params = block_params(&pipeline, cfg, 0);
    let sum = find_arith(&pipeline, cfg, |op| match op {
        Arith::Add { result, .. } => Some(SSAValue::from(*result)),
        _ => None,
    });
    assert!(result.is_demanded(sum));
    assert!(result.is_demanded(params[0]));
    assert!(result.is_demanded(params[1]));
}

#[test]
fn dead_result_leaves_operands_dead() {
    let pipeline = parse(DEAD_RESULT_PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

    let params = block_params(&pipeline, cfg, 0);
    let sum = find_arith(&pipeline, cfg, |op| match op {
        Arith::Add { result, .. } => Some(SSAValue::from(*result)),
        _ => None,
    });
    assert!(!result.is_demanded(sum), "the add result is never used");
    assert!(result.is_demanded(params[0]), "ret %a demands the param");
    assert!(
        !result.is_demanded(params[1]),
        "a pure statement with a dead result demands nothing"
    );
}

// ===========================================================================
// Classic (dense, per-point) liveness — deliberately conventional semantics:
// every use gens, purity-irrelevant, so dead code's operands DO appear in the
// per-point sets. The strong per-point view is the composition
// `classic ∩ demanded`, which reproduces neededness.
// ===========================================================================

use kirin_liveness::{LiveSet, analyze_dense};

/// The `index`-th block of `cfg`.
fn nth_block(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::Cfg,
    index: usize,
) -> kirin_ir::Block {
    let stage_id = pipeline.stage_by_name("test").expect("stage @test exists");
    let stage = pipeline.stage(stage_id).expect("stage info");
    cfg.blocks(stage).nth(index).expect("block index in range")
}

/// The statement whose definition matches `select`.
fn find_stmt(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::Cfg,
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
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_dense(&pipeline, stage, cfg).expect("analysis succeeds");

    let entry_params = block_params(&pipeline, cfg, 0);
    let (x, cond) = (entry_params[0], entry_params[1]);
    let then_param = block_params(&pipeline, cfg, 1)[0];
    let else_param = block_params(&pipeline, cfg, 2)[0];
    let entry = nth_block(&pipeline, cfg, 0);
    let then_block = nth_block(&pipeline, cfg, 1);
    let else_block = nth_block(&pipeline, cfg, 2);

    // live_in(entry): %x (used by add and both edges) and %cond (branch use).
    assert_eq!(result.live_in(entry), Some(&live_set(&[x, cond])));
    // live_out(entry): both successors' live-ins mapped across the edges —
    // {%a} → {%x}, {%b} → {%x}; the branch condition is a terminator *use*,
    // not part of the boundary set.
    assert_eq!(result.live_out(entry), Some(&live_set(&[x])));
    assert_eq!(result.live_in(then_block), Some(&live_set(&[then_param])));
    assert_eq!(result.live_out(then_block), Some(&live_set(&[])));
    assert_eq!(result.live_in(else_block), Some(&live_set(&[else_param])));
    assert_eq!(result.live_out(else_block), Some(&live_set(&[])));
}

#[test]
fn classic_per_point_sets_gen_dead_uses() {
    let pipeline = parse(DEAD_RESULT_PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let result = analyze_dense(&pipeline, stage, cfg).expect("analysis succeeds");

    let params = block_params(&pipeline, cfg, 0);
    let (a, b) = (params[0], params[1]);
    let add = find_stmt(&pipeline, cfg, |definition| {
        matches!(definition, ArithFunctionLanguage::Arith(Arith::Add { .. }))
    });

    // Classic semantics: the dead add still GENS its operands, so %b is live
    // before it — this is the conventional per-point meaning (the old strong
    // expectations were demand projections, not dense liveness).
    assert_eq!(result.live_before(add), Some(&live_set(&[a, b])));
    assert_eq!(result.live_after(add), Some(&live_set(&[a])));
}

#[test]
fn strong_per_point_sets_are_classic_intersect_demanded() {
    let pipeline = parse(DEAD_RESULT_PROGRAM);
    let (stage, cfg) = main_cfg(&pipeline);
    let dense = analyze_dense(&pipeline, stage, cfg).expect("dense analysis succeeds");
    let demand = analyze_demand(&pipeline, stage, cfg).expect("demand analysis succeeds");

    let params = block_params(&pipeline, cfg, 0);
    let (a, b) = (params[0], params[1]);
    let add = find_stmt(&pipeline, cfg, |definition| {
        matches!(definition, ArithFunctionLanguage::Arith(Arith::Add { .. }))
    });

    // The composition recovers the strong (needed) per-point view: %b is
    // classically live before the dead add but not demanded, so it drops out.
    let strong = dense
        .strong_live_before(add, &demand)
        .expect("point reconstructed");
    assert_eq!(strong, live_set(&[a]));
    assert!(!strong.contains(b));
}
