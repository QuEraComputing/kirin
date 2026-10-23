//! Integration tests: strong liveness (sparse backward demand) over real
//! parsed programs, with transfer dispatched through the dialects'
//! `Interpretable<I, StrongDemand>` rules.

use kirin::prelude::{GetInfo, ParsePipelineText, Pipeline, SSAValue, StageInfo};
use kirin_arith::Arith;
use kirin_demand::{Demand, analyze_demand};
use kirin_interpreter::{Body, Callee};
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

const DEAD_EDGE_ARG_PROGRAM: &str = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 cfg {
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

/// Find an `Arith` statement in `cfg` matching `select`, returning what it
/// selects (e.g. the result and operands of the one `add`).
fn find_arith<R>(
    pipeline: &Pipeline<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::CFG,
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
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result =
        analyze_demand(&pipeline, caller_stage, callee, [Demand::Live]).expect("analysis succeeds");
    let (_, Body::CFG(cfg)) = result.root_scope() else {
        panic!("@main resolves to a CFG body")
    };

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
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result =
        analyze_demand(&pipeline, caller_stage, callee, [Demand::Live]).expect("analysis succeeds");
    let (_, Body::CFG(cfg)) = result.root_scope() else {
        panic!("@main resolves to a CFG body")
    };

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

specialize @test fn @main(i64) -> i64 cfg {
  ^entry(%x: i64) {
    ret %x;
  }
}
"#;

const DEMANDED_RESULT_PROGRAM: &str = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 cfg {
  ^entry(%a: i64, %b: i64) {
    %s = add %a, %b -> i64;
    ret %s;
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

#[test]
fn terminator_operands_become_demanded() {
    let pipeline = parse(RET_PARAM_PROGRAM);
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result =
        analyze_demand(&pipeline, caller_stage, callee, [Demand::Live]).expect("analysis succeeds");
    let (_, Body::CFG(cfg)) = result.root_scope() else {
        panic!("@main resolves to a CFG body")
    };

    let x = block_params(&pipeline, cfg, 0)[0];
    assert!(result.is_demanded(x));
}

#[test]
fn demanded_result_marks_operands_demanded() {
    let pipeline = parse(DEMANDED_RESULT_PROGRAM);
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result =
        analyze_demand(&pipeline, caller_stage, callee, [Demand::Live]).expect("analysis succeeds");
    let (_, Body::CFG(cfg)) = result.root_scope() else {
        panic!("@main resolves to a CFG body")
    };

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
    let (caller_stage, callee) = function_root(&pipeline, "main");
    let result =
        analyze_demand(&pipeline, caller_stage, callee, [Demand::Live]).expect("analysis succeeds");
    let (_, Body::CFG(cfg)) = result.root_scope() else {
        panic!("@main resolves to a CFG body")
    };

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
