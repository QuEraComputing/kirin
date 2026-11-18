//! Integration tests: strong liveness (sparse backward demand) over real
//! parsed programs, with transfer dispatched through the dialects'
//! `Interpretable<I, StrongDemand>` rules.

use kirin::prelude::{GetInfo, ParseProgramText, Program, SSAValue, StageInfo};
use kirin_arith::Arith;
use kirin_demand::{Demand, DemandAnalysis};
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

fn parse(source: &str) -> Program<StageInfo<ArithFunctionLanguage>> {
    let mut program: Program<StageInfo<ArithFunctionLanguage>> = Program::new();
    ParseProgramText::parse(&mut program, source).expect("program parses");
    program
}

/// The caller stage and canonical callable root for `function_name`.
fn function_root(
    program: &Program<StageInfo<ArithFunctionLanguage>>,
    function_name: &str,
) -> (kirin::prelude::CompileStage, Callee) {
    let stage_id = program.stage_by_name("test").expect("stage @test exists");
    let function = program
        .lookup_function_by_name(function_name)
        .unwrap_or_else(|| panic!("function @{function_name} exists"));
    (stage_id, Callee::Function(function))
}

/// The parameters of the `index`-th block of `cfg`, as SSA values.
fn block_params(
    program: &Program<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::CFG,
    index: usize,
) -> Vec<SSAValue> {
    let stage_id = program.stage_by_name("test").expect("stage @test exists");
    let stage = program.stage(stage_id).expect("stage info");
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
    program: &Program<StageInfo<ArithFunctionLanguage>>,
    cfg: kirin_ir::CFG,
    select: impl Fn(&Arith<kirin_arith::ArithType>) -> Option<R>,
) -> R {
    let stage_id = program.stage_by_name("test").expect("stage @test exists");
    let stage = program.stage(stage_id).expect("stage info");
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
    let program = parse(PROGRAM);
    let (caller_stage, callee) = function_root(&program, "main");
    let result = DemandAnalysis::<_>::new(&program)
        .analyze(caller_stage, callee, [Demand::Live])
        .expect("analysis succeeds");
    let root = result.callable_of(result.root()).expect("root is reached");
    let Ok(Body::CFG(cfg)) = root.body(&program) else {
        panic!("@main resolves to a CFG body")
    };

    let entry_params = block_params(&program, cfg, 0);
    let (x, cond) = (entry_params[0], entry_params[1]);
    let then_param = block_params(&program, cfg, 1)[0];
    let else_param = block_params(&program, cfg, 2)[0];

    // The dead `add` result is never demanded — its rule is purity-aware, so
    // it contributes no operand demand of its own.
    let (dead, add_lhs) = find_arith(&program, cfg, |op| match op {
        Arith::Add { lhs, result, .. } => Some((SSAValue::from(*result), *lhs)),
        _ => None,
    });
    assert!(result.fact(result.root(), dead.into()) != Some(&Demand::Live));
    assert_eq!(add_lhs, x, "the add reads the entry parameter");

    // Roots and their transitive demands: return operands, the branch
    // condition, and the edge arguments feeding demanded successor params.
    assert!(
        result.fact(result.root(), cond.into()) == Some(&Demand::Live),
        "branch condition is a root"
    );
    assert!(
        result.fact(result.root(), then_param.into()) == Some(&Demand::Live),
        "ret %a demands ^then's param"
    );
    assert!(
        result.fact(result.root(), else_param.into()) == Some(&Demand::Live),
        "neg feeds ret in ^else"
    );
    assert!(
        result.fact(result.root(), x.into()) == Some(&Demand::Live),
        "both edges pass %x into demanded params"
    );

    let neg = find_arith(&program, cfg, |op| match op {
        Arith::Neg { result, .. } => Some(SSAValue::from(*result)),
        _ => None,
    });
    assert!(
        result.fact(result.root(), neg.into()) == Some(&Demand::Live),
        "ret %neg demands the neg result"
    );
}

#[test]
fn unused_successor_block_argument_does_not_keep_edge_arg_live() {
    let program = parse(DEAD_EDGE_ARG_PROGRAM);
    let (caller_stage, callee) = function_root(&program, "main");
    let result = DemandAnalysis::<_>::new(&program)
        .analyze(caller_stage, callee, [Demand::Live])
        .expect("analysis succeeds");
    let root = result.callable_of(result.root()).expect("root is reached");
    let Ok(Body::CFG(cfg)) = root.body(&program) else {
        panic!("@main resolves to a CFG body")
    };

    let entry_params = block_params(&program, cfg, 0);
    let (live, dead, cond) = (entry_params[0], entry_params[1], entry_params[2]);
    let then_param = block_params(&program, cfg, 1)[0];
    let else_param = block_params(&program, cfg, 2)[0];

    // `^else` returns `%live` directly (a dominated cross-block use), never
    // touching its own parameter — so the `%dead` edge argument stays dead.
    assert!(result.fact(result.root(), live.into()) == Some(&Demand::Live));
    assert!(result.fact(result.root(), cond.into()) == Some(&Demand::Live));
    assert!(result.fact(result.root(), then_param.into()) == Some(&Demand::Live));
    assert!(
        result.fact(result.root(), else_param.into()) != Some(&Demand::Live),
        "^else's param is unused"
    );
    assert!(
        result.fact(result.root(), dead.into()) != Some(&Demand::Live),
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
    let program = parse(RET_PARAM_PROGRAM);
    let (caller_stage, callee) = function_root(&program, "main");
    let result = DemandAnalysis::<_>::new(&program)
        .analyze(caller_stage, callee, [Demand::Live])
        .expect("analysis succeeds");
    let root = result.callable_of(result.root()).expect("root is reached");
    let Ok(Body::CFG(cfg)) = root.body(&program) else {
        panic!("@main resolves to a CFG body")
    };

    let x = block_params(&program, cfg, 0)[0];
    assert!(result.fact(result.root(), x.into()) == Some(&Demand::Live));
}

#[test]
fn demanded_result_marks_operands_demanded() {
    let program = parse(DEMANDED_RESULT_PROGRAM);
    let (caller_stage, callee) = function_root(&program, "main");
    let result = DemandAnalysis::<_>::new(&program)
        .analyze(caller_stage, callee, [Demand::Live])
        .expect("analysis succeeds");
    let root = result.callable_of(result.root()).expect("root is reached");
    let Ok(Body::CFG(cfg)) = root.body(&program) else {
        panic!("@main resolves to a CFG body")
    };

    let params = block_params(&program, cfg, 0);
    let sum = find_arith(&program, cfg, |op| match op {
        Arith::Add { result, .. } => Some(SSAValue::from(*result)),
        _ => None,
    });
    assert!(result.fact(result.root(), sum.into()) == Some(&Demand::Live));
    assert!(result.fact(result.root(), params[0].into()) == Some(&Demand::Live));
    assert!(result.fact(result.root(), params[1].into()) == Some(&Demand::Live));
}

#[test]
fn dead_result_leaves_operands_dead() {
    let program = parse(DEAD_RESULT_PROGRAM);
    let (caller_stage, callee) = function_root(&program, "main");
    let result = DemandAnalysis::<_>::new(&program)
        .analyze(caller_stage, callee, [Demand::Live])
        .expect("analysis succeeds");
    let root = result.callable_of(result.root()).expect("root is reached");
    let Ok(Body::CFG(cfg)) = root.body(&program) else {
        panic!("@main resolves to a CFG body")
    };

    let params = block_params(&program, cfg, 0);
    let sum = find_arith(&program, cfg, |op| match op {
        Arith::Add { result, .. } => Some(SSAValue::from(*result)),
        _ => None,
    });
    assert!(
        result.fact(result.root(), sum.into()) != Some(&Demand::Live),
        "the add result is never used"
    );
    assert!(
        result.fact(result.root(), params[0].into()) == Some(&Demand::Live),
        "ret %a demands the param"
    );
    assert!(
        result.fact(result.root(), params[1].into()) != Some(&Demand::Live),
        "a pure statement with a dead result demands nothing"
    );
}
