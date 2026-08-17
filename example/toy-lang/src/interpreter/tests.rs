use kirin::prelude::{GetInfo, HasStageInfo, ParsePipelineText, Pipeline, Signature, StageInfo};
use kirin_arith::ArithType;
use kirin_function::{Call, Function, Return};

use crate::interpreter::{analyze_constprop, run_i64, run_source_i64};
use crate::language::LowLevel;
use crate::stage::Stage;

type ConstProp = kirin_constprop::ConstPropValue;

const ADD_LOWERED: &str = r#"
stage @lowered fn @add(i64, i64) -> i64;

specialize @lowered fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

const BRANCH_LOWERED: &str = r#"
stage @lowered fn @sign(i64) -> i64;

specialize @lowered fn @sign(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^neg() else=^pos();
  }
  ^neg() {
    %one = constant 1 -> i64;
    ret %one;
  }
  ^pos() {
    %zero2 = constant 0 -> i64;
    ret %zero2;
  }
}
"#;

const SAME_BRANCH_LOWERED: &str = r#"
stage @lowered fn @same(i64) -> i64;
specialize @lowered fn @same(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^lhs() else=^rhs();
  }
  ^lhs() { %one = constant 1 -> i64; ret %one; }
  ^rhs() { %also_one = constant 1 -> i64; ret %also_one; }
}
"#;

// A successor block that directly uses `%v` — defined in the dominating `^entry`
// block and **not** passed as a block argument (a direct dominated cross-block
// SSA use, which Kirin supports). Guards that block-owner convergence reschedules
// on directly-read external facts, not only on block-parameter edges.
const CROSS_BLOCK_DIRECT_USE: &str = r#"
stage @lowered fn @cross(i64) -> i64;

specialize @lowered fn @cross(i64) -> i64 {
  ^entry(%x: i64) {
    %v = add %x, %x -> i64;
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^then() else=^else();
  }
  ^then() {
    ret %v;
  }
  ^else() {
    ret %v;
  }
}
"#;

// A loop where a value defined in the loop head (`%doubled`, NOT a block param)
// is read directly in the exit block, and *rises* across iterations as the
// loop-carried `%acc` widens to `Top`. The exit block has no parameters, so its
// block-entry never changes — under CFG-successor-edge scheduling alone it runs
// once (reading the early `%doubled`) and is never rerun when `%doubled` rises.
// Sound analysis must still return `Top`. This is the M3b acceptance stress test
// for value/def-use deps; it exposes the gap the retained `AbstractCFGFrame`
// worklist has, which M3b's Block owners + value-reader deps close.
const LOOP_CARRIED_CROSS_BLOCK_RISE: &str = r#"
stage @lowered fn @rise(i64) -> i64;

specialize @lowered fn @rise(i64) -> i64 {
  ^entry(%n: i64) {
    %z = constant 0 -> i64;
    br ^head(%z);
  }
  ^head(%acc: i64) {
    %doubled = add %acc, %acc -> i64;
    %lt = lt %acc, %n -> i64;
    cond_br %lt then=^body() else=^exit();
  }
  ^body() {
    %one = constant 1 -> i64;
    %next = add %acc, %one -> i64;
    br ^head(%next);
  }
  ^exit() {
    ret %doubled;
  }
}
"#;

const SOURCE_FOR_CARRIED_STABLE: &str = r#"
stage @source fn @stable(i64, i64, i64) -> i64;

specialize @source fn @stable(i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64) {
    %init = constant 0 -> i64;
    %sum = for %lo in %lo..%hi step %s iter_args(%init) do ^body(%i: i64, %acc: i64) {
      yield %acc;
    } -> i64;
    ret %sum;
  }
}
"#;

// An `scf.if` whose condition is the (unknown-under-constprop) argument, both
// arms yielding the *same* constant. Exercises `AbstractScfIfFrame` exploring
// both arms and joining identical finishes -> `Const(1)`.
const SOURCE_IF_SAME_CONST: &str = r#"
stage @source fn @if_same(i64) -> i64;

specialize @source fn @if_same(i64) -> i64 {
  ^entry(%cond: i64) {
    %result = if %cond then ^then() {
      %a = constant 1 -> i64;
      yield %a;
    } else ^else() {
      %b = constant 1 -> i64;
      yield %b;
    } -> i64;
    ret %result;
  }
}
"#;

// The same shape, but the arms yield *different* constants: joining them is
// `Top`.
const SOURCE_IF_DIFF_CONST: &str = r#"
stage @source fn @if_diff(i64) -> i64;

specialize @source fn @if_diff(i64) -> i64 {
  ^entry(%cond: i64) {
    %result = if %cond then ^then() {
      %a = constant 1 -> i64;
      yield %a;
    } else ^else() {
      %b = constant 2 -> i64;
      yield %b;
    } -> i64;
    ret %result;
  }
}
"#;

const CROSS_STAGE_CALLS: &str = r#"
stage @source fn @source_to_lowered_to_source(i64) -> i64;
stage @source fn @low_then_high(i64) -> i64;
stage @source fn @source_abs(i64) -> i64;
stage @lowered fn @low_then_high(i64) -> i64;
stage @lowered fn @source_abs(i64) -> i64;

specialize @source fn @source_to_lowered_to_source(i64) -> i64 {
  ^entry(%x: i64) {
    %result = call.named @low_then_high(%x) -> i64;
    ret %result;
  }
}

specialize @source fn @source_abs(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    %result = if %is_neg then ^then() {
      %negated = neg %x -> i64;
      yield %negated;
    } else ^else() {
      yield %x;
    } -> i64;
    ret %result;
  }
}

specialize @lowered fn @low_then_high(i64) -> i64 {
  ^entry(%x: i64) {
    %abs = call.named @source_abs(%x) -> i64;
    %one = constant 1 -> i64;
    %result = add %abs, %one -> i64;
    ret %result;
  }
}
"#;

const CROSS_STAGE_SPECIALIZED_CALLS: &str = r#"
stage @source fn @source_direct_specialized(i64) -> i64;
stage @source fn @dual_impl(i64) -> i64;
stage @lowered fn @dual_impl(i64) -> i64;

specialize @source fn @dual_impl(i64) -> i64 {
  ^entry(%x: i64) {
    %one = constant 1 -> i64;
    %result = add %x, %one -> i64;
    ret %result;
  }
}

specialize @lowered fn @dual_impl(i64) -> i64 {
  ^entry(%x: i64) {
    %hundred = constant 100 -> i64;
    %result = add %x, %hundred -> i64;
    ret %result;
  }
}
"#;

fn build_pipeline(src: &str) -> Pipeline<Stage> {
    let mut pipeline = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, src).expect("parse failed");
    pipeline
}

fn build_cross_stage_specialized_pipeline() -> Pipeline<Stage> {
    let mut pipeline = build_pipeline(CROSS_STAGE_SPECIALIZED_CALLS);
    let source_stage = pipeline.stage_by_name("source").unwrap();
    let lowered_stage = pipeline.stage_by_name("lowered").unwrap();
    let caller = pipeline
        .resolve_staged_function("source_direct_specialized", source_stage)
        .unwrap();
    let lowered_dual_impl = pipeline
        .resolve_staged_function("dual_impl", lowered_stage)
        .unwrap();
    let lowered_stage_meta = pipeline.stage(lowered_stage).unwrap();
    let lowered_info: &StageInfo<LowLevel> = lowered_stage_meta.try_stage_info().unwrap();
    let lowered_specialized = lowered_dual_impl
        .get_info(lowered_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();

    let Stage::Source(source_info) = pipeline.stage_mut(source_stage).unwrap() else {
        unreachable!("source stage id resolved to a non-source stage");
    };
    source_info.with_builder(|builder| {
        let x = builder.block_argument().index(0);
        let call = Call::<ArithType>::build(builder)
            .in_stage(lowered_stage)
            .specialized(lowered_specialized)
            .args(vec![x])
            .results(1)
            .insert();
        let ret = Return::<ArithType>::new(builder, vec![call.results[0].into()]);
        let block = builder
            .block()
            .argument(ArithType::I64)
            .stmt(call)
            .terminator(ret)
            .new();
        let cfg = builder.cfg().add_block(block).new();
        let definition = Function::<ArithType>::new(
            builder,
            cfg,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        );
        builder
            .specialize()
            .staged_func(caller)
            .definition(definition)
            .new()
            .unwrap();
    });

    pipeline
}

#[test]
fn runs_source_add() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result = run_source_i64(&pipeline, "main", &[3, 5]).unwrap();
    assert_eq!(result, 8);
}

#[test]
fn runs_source_branching() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    assert_eq!(run_source_i64(&pipeline, "abs", &[-7]).unwrap(), 7);
    assert_eq!(run_source_i64(&pipeline, "abs", &[7]).unwrap(), 7);
}

#[test]
fn runs_source_recursive_factorial() {
    let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));
    let result = run_source_i64(&pipeline, "factorial", &[5]).unwrap();
    assert_eq!(result, 120);
}

#[test]
fn constprop_source_recursive_factorial() {
    // Bounded arg-tuple context sensitivity unfolds factorial(5) → 4 → 3 → 2 → 1
    // under distinct summary keys, so the analysis folds the result back
    // precisely instead of collapsing the entry argument to Top.
    let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));
    let result =
        analyze_constprop(&pipeline, "source", "factorial", &[ConstProp::Const(5)]).unwrap();
    assert_eq!(result, ConstProp::Const(120));
}

#[test]
fn constprop_source_recursive_factorial_unknown_is_top() {
    // An unknown argument keys the single shared (Unknown) context, so the
    // recursive call lands on the *same* key. The self-dependency fix (a
    // same-key caller re-enqueues its own summary as the return value rises)
    // converges this soundly to `Top` — not a bogus `Const` from seeing only
    // the base case — and terminates.
    let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));
    let result = analyze_constprop(&pipeline, "source", "factorial", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn runs_source_recursive_fibonacci() {
    let pipeline = build_pipeline(include_str!("../../programs/fibonacci.kirin"));
    let result = run_source_i64(&pipeline, "fib", &[10]).unwrap();
    assert_eq!(result, 55);
}

#[test]
fn constprop_source_recursive_fibonacci() {
    // Fibonacci recursion is an overlapping-subproblem DAG: fib(n-1) and fib(n-2)
    // both recompute fib(n-3), fib(n-4), ... Bounded arg-tuple context sensitivity
    // keys each fib(k) under a distinct summary, so every constant argument is
    // analyzed once and folded back precisely (fib(10) = 55) — the two recursive
    // call sites reuse the memoized per-constant summaries instead of joining
    // their entry arguments to Top.
    let pipeline = build_pipeline(include_str!("../../programs/fibonacci.kirin"));
    let result = analyze_constprop(&pipeline, "source", "fib", &[ConstProp::Const(10)]).unwrap();
    assert_eq!(result, ConstProp::Const(55));
}

#[test]
fn constprop_source_recursive_fibonacci_unknown_is_top() {
    // An unknown argument keys the single shared (Unknown) context. Both
    // recursive call sites (fib(n-1), fib(n-2)) land back on that same key, so
    // the self-dependency fix re-enqueues the summary as its return value rises
    // and converges soundly to `Top` (and terminates), rather than reporting a
    // bogus `Const` from seeing only the base case.
    let pipeline = build_pipeline(include_str!("../../programs/fibonacci.kirin"));
    let result = analyze_constprop(&pipeline, "source", "fib", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_source_add() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result = analyze_constprop(
        &pipeline,
        "source",
        "main",
        &[ConstProp::Const(3), ConstProp::Const(5)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(8));
}

#[test]
fn constprop_source_add_with_unknown() {
    let pipeline = build_pipeline(include_str!("../../programs/add.kirin"));
    let result = analyze_constprop(
        &pipeline,
        "source",
        "main",
        &[ConstProp::Top, ConstProp::Const(5)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_direct_dominated_cross_block_use() {
    // `^then`/`^else` read `%v` directly from the dominating `^entry` block
    // without it being a block argument. Constprop must propagate the fact across
    // the CFG edge: `%v = %x + %x`, so `@cross(Const(3)) == Const(6)`, and an
    // unknown input widens to `Top`. (Regression for M3b block-owner convergence:
    // a directly-read external fact must reach the reading block.)
    let pipeline = build_pipeline(CROSS_BLOCK_DIRECT_USE);
    assert_eq!(
        analyze_constprop(&pipeline, "lowered", "cross", &[ConstProp::Const(3)]).unwrap(),
        ConstProp::Const(6)
    );
    assert_eq!(
        analyze_constprop(&pipeline, "lowered", "cross", &[ConstProp::Top]).unwrap(),
        ConstProp::Top
    );
}

#[test]
fn constprop_loop_carried_cross_block_rise_is_top() {
    let pipeline = build_pipeline(LOOP_CARRIED_CROSS_BLOCK_RISE);
    // `%acc` widens to Top, so `%doubled = %acc + %acc` is Top, and `ret %doubled`
    // must be Top. (M3a returns a stale early constant — the gap M3b closes.)
    assert_eq!(
        analyze_constprop(&pipeline, "lowered", "rise", &[ConstProp::Top]).unwrap(),
        ConstProp::Top
    );
}

#[test]
fn constprop_source_known_branch() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    assert_eq!(
        analyze_constprop(&pipeline, "source", "abs", &[ConstProp::Const(-7)]).unwrap(),
        ConstProp::Const(7)
    );
    assert_eq!(
        analyze_constprop(&pipeline, "source", "abs", &[ConstProp::Const(7)]).unwrap(),
        ConstProp::Const(7)
    );
}

#[test]
fn constprop_source_unknown_branch_joins_yields() {
    let pipeline = build_pipeline(include_str!("../../programs/branching.kirin"));
    let result = analyze_constprop(&pipeline, "source", "abs", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

// `AbstractScfIfFrame` owns the "explore both arms + join" behavior that used to
// live in the removed `SparseForwardEffect::EnterAny` / framework alternatives frame.
// With an unknown condition both arms are explored and their finishes joined.

#[test]
fn constprop_unknown_if_same_constant_joins_to_that_constant() {
    // if %unknown { yield 1 } else { yield 1 } -> join(Const(1), Const(1)) = Const(1)
    let pipeline = build_pipeline(SOURCE_IF_SAME_CONST);
    let result = analyze_constprop(&pipeline, "source", "if_same", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Const(1));
}

#[test]
fn constprop_unknown_if_different_constants_join_to_top() {
    // if %unknown { yield 1 } else { yield 2 } -> join(Const(1), Const(2)) = Top
    let pipeline = build_pipeline(SOURCE_IF_DIFF_CONST);
    let result = analyze_constprop(&pipeline, "source", "if_diff", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_source_for_keeps_stable_carried_value() {
    let pipeline = build_pipeline(SOURCE_FOR_CARRIED_STABLE);
    let result = analyze_constprop(
        &pipeline,
        "source",
        "stable",
        &[ConstProp::Const(0), ConstProp::Top, ConstProp::Const(1)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(0));
}

#[test]
fn runs_cross_stage_source_to_lowered_to_source_concretely() {
    let pipeline = build_pipeline(CROSS_STAGE_CALLS);

    // source_to_lowered_to_source(-7) calls @low_then_high (bodied at
    // lowered) which calls @source_abs (bodied at source). The cross-stage
    // linker must route both calls to their bodied stages.
    let result = run_i64(&pipeline, "source", "source_to_lowered_to_source", &[-7]).unwrap();
    assert_eq!(result, 8);

    let lowered_result = run_i64(&pipeline, "lowered", "low_then_high", &[-4]).unwrap();
    assert_eq!(lowered_result, 5);
}

#[test]
fn constprop_cross_stage_calls_between_source_and_lowered() {
    let pipeline = build_pipeline(CROSS_STAGE_CALLS);

    let source_result = analyze_constprop(
        &pipeline,
        "source",
        "source_to_lowered_to_source",
        &[ConstProp::Const(-7)],
    )
    .unwrap();
    assert_eq!(source_result, ConstProp::Const(8));

    let lowered_result = analyze_constprop(
        &pipeline,
        "lowered",
        "low_then_high",
        &[ConstProp::Const(-4)],
    )
    .unwrap();
    assert_eq!(lowered_result, ConstProp::Const(5));
}

#[test]
fn constprop_cross_stage_call_specialized_uses_direct_target() {
    let pipeline = build_cross_stage_specialized_pipeline();

    let result = analyze_constprop(
        &pipeline,
        "source",
        "source_direct_specialized",
        &[ConstProp::Const(5)],
    )
    .unwrap();

    assert_eq!(result, ConstProp::Const(105));
}

#[test]
fn constprop_lowered_add() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = analyze_constprop(
        &pipeline,
        "lowered",
        "add",
        &[ConstProp::Const(2), ConstProp::Const(3)],
    )
    .unwrap();
    assert_eq!(result, ConstProp::Const(5));
}

#[test]
fn constprop_lowered_known_cf_branch() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    assert_eq!(
        analyze_constprop(&pipeline, "lowered", "sign", &[ConstProp::Const(-3)]).unwrap(),
        ConstProp::Const(1)
    );
    assert_eq!(
        analyze_constprop(&pipeline, "lowered", "sign", &[ConstProp::Const(5)]).unwrap(),
        ConstProp::Const(0)
    );
}

#[test]
fn constprop_lowered_unknown_cf_branch_returns_top() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_constprop(&pipeline, "lowered", "sign", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Top);
}

#[test]
fn constprop_lowered_unknown_cf_branch_joins_matching_returns() {
    let pipeline = build_pipeline(SAME_BRANCH_LOWERED);
    let result = analyze_constprop(&pipeline, "lowered", "same", &[ConstProp::Top]).unwrap();
    assert_eq!(result, ConstProp::Const(1));
}

/// Analysis-author surface for a custom abstract policy budget.
mod advanced {
    use kirin_constprop::{ConstPropContext, ConstPropValue};
    use kirin_interpreter::{SameStageLinker, engine::expect_single};

    use super::build_pipeline;

    use crate::interpreter::ToyError;

    // --- A capped custom abstract policy -----------------------------------

    #[test]
    fn constprop_context_budget_overflow_falls_back_to_top() {
        // Budget 2 admits the [5] and [4] contexts; [3]/[2]/[1] overflow to the
        // shared `Unknown` context (joined → Top). The result is soundly `Top`
        // (capping degrades precision, not soundness) — and it terminates,
        // which exercises the same-key self-dependency convergence.
        let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));
        let mut analysis = crate::interpreter::ToyConstProp::new(&pipeline)
            .with_policy(ConstPropContext::with_budget(2))
            .with_linker(SameStageLinker);
        let result = expect_single::<ConstPropValue, ToyError>(
            analysis
                .analyze_by_name("source", "factorial", [ConstPropValue::Const(5)])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result, ConstPropValue::Top);
    }
}

// ===========================================================================
// Strong liveness (sparse backward demand) over source-stage programs with
// structured control flow — transfer dispatched through the dialects'
// `Interpretable<I, StrongDemand>` rules, scf handled by rules alone (the
// value worklist converges loop-carried demand without frames).
// ===========================================================================

mod demand {
    use std::collections::HashSet;

    use kirin::prelude::{
        CFG, CompileStage, GetInfo, HasBlocks, HasCFG, HasCFGBody, HasDigraphs, HasResults,
        HasUngraphs, ParsePipelineText, Pipeline, SSAValue, Statement,
    };
    use kirin_arith::{Arith, ArithValue};
    use kirin_function::Lexical;
    use kirin_interpreter::Body;
    use kirin_liveness::analyze_demand;

    use crate::language::HighLevel;
    use crate::stage::Stage;

    pub(super) fn parse(program: &str) -> Pipeline<Stage> {
        let mut pipeline: Pipeline<Stage> = Pipeline::new();
        ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
        pipeline
    }

    /// The source stage id, its info, and the body cfg of `name`.
    pub(super) fn source_cfg(pipeline: &Pipeline<Stage>, name: &str) -> (CompileStage, CFG) {
        let stage_id = pipeline.stage_by_name("source").expect("source stage");
        let Stage::Source(info) = pipeline.stage(stage_id).expect("stage info") else {
            panic!("source stage holds HighLevel");
        };
        let sf = pipeline
            .resolve_staged_function(name, stage_id)
            .expect("staged function");
        let sf_info = sf.get_info(info).expect("staged function info");
        let definition = *sf_info.specializations()[0].definition();
        let cfg = match definition.definition(info) {
            HighLevel::Lexical(Lexical::Function(function)) => *function.cfg(),
            other => panic!("expected a function definition, got {other:?}"),
        };
        (stage_id, cfg)
    }

    /// The parameters of the CFG's entry block.
    pub(super) fn entry_params(pipeline: &Pipeline<Stage>, cfg: CFG) -> Vec<SSAValue> {
        let stage_id = pipeline.stage_by_name("source").expect("source stage");
        let Stage::Source(info) = pipeline.stage(stage_id).expect("stage info") else {
            panic!("source stage holds HighLevel");
        };
        let block = cfg.blocks(info).next().expect("entry block");
        block
            .expect_info(info)
            .arguments
            .iter()
            .copied()
            .map(SSAValue::from)
            .collect()
    }

    /// Find something by walking statement definitions anywhere in the CFG,
    /// including bodies nested under statements.
    fn find_in_body<R>(
        pipeline: &Pipeline<Stage>,
        cfg: CFG,
        mut select: impl FnMut(Statement, &HighLevel) -> Option<R>,
    ) -> R {
        let stage_id = pipeline.stage_by_name("source").expect("source stage");
        let Stage::Source(info) = pipeline.stage(stage_id).expect("stage info") else {
            panic!("source stage holds HighLevel");
        };

        let mut bodies = vec![Body::CFG(cfg)];
        let mut visited = HashSet::new();
        while let Some(body) = bodies.pop() {
            if !visited.insert(body) {
                continue;
            }

            let statements: Vec<Statement> = match body {
                Body::CFG(cfg) => {
                    bodies.extend(cfg.blocks(info).map(Body::Block));
                    continue;
                }
                Body::Block(block) => {
                    let mut statements: Vec<Statement> = block.statements(info).collect();
                    if let Some(terminator) = block.terminator(info) {
                        statements.push(terminator);
                    }
                    statements
                }
                Body::DiGraph(graph) => graph
                    .expect_info(info)
                    .graph()
                    .node_weights()
                    .copied()
                    .collect(),
                Body::UnGraph(graph) => graph
                    .expect_info(info)
                    .graph()
                    .node_weights()
                    .copied()
                    .collect(),
            };

            for statement in statements {
                let definition = statement.definition(info);
                if let Some(value) = select(statement, definition) {
                    return value;
                }
                bodies.extend(definition.blocks().copied().map(Body::Block));
                bodies.extend(definition.cfgs().copied().map(Body::CFG));
                bodies.extend(definition.digraphs().copied().map(Body::DiGraph));
                bodies.extend(definition.ungraphs().copied().map(Body::UnGraph));
            }
        }
        panic!("no matching statement in cfg");
    }

    pub(super) fn find_value<R>(
        pipeline: &Pipeline<Stage>,
        cfg: CFG,
        select: impl FnMut(&HighLevel) -> Option<R>,
    ) -> R {
        let mut select = select;
        find_in_body(pipeline, cfg, |_, definition| select(definition))
    }

    pub(super) fn find_statement(
        pipeline: &Pipeline<Stage>,
        cfg: CFG,
        select: impl FnMut(&HighLevel) -> bool,
    ) -> Statement {
        let mut select = select;
        find_in_body(pipeline, cfg, |statement, definition| {
            select(definition).then_some(statement)
        })
    }

    /// The result of the `constant <value> -> i64` statement.
    pub(super) fn constant_result(pipeline: &Pipeline<Stage>, cfg: CFG, value: i64) -> SSAValue {
        find_value(pipeline, cfg, |definition| match definition {
            HighLevel::Constant(constant) if constant.value == ArithValue::I64(value) => {
                Some(constant.result.into())
            }
            _ => None,
        })
    }

    const IF_BODY_DEMAND: &str = r#"
stage @source fn @if_body(i64) -> i64;

specialize @source fn @if_body(i64) -> i64 {
  ^entry(%cond: i64) {
    %result = if %cond then ^then() {
      %a = constant 1 -> i64;
      %junk = constant 9 -> i64;
      yield %a;
    } else ^else() {
      %b = constant 2 -> i64;
      yield %b;
    } -> i64;
    ret %result;
  }
}
"#;

    /// Values used only inside scf bodies are demanded when the result is —
    /// the old structural liveness never looked inside bodies at all — while
    /// a body-interior dead statement stays dead.
    #[test]
    fn scf_if_body_demand_follows_result_demand() {
        let pipeline = parse(IF_BODY_DEMAND);
        let (stage, cfg) = source_cfg(&pipeline, "if_body");
        let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

        let cond = entry_params(&pipeline, cfg)[0];
        let if_result = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Structured(_) => definition.results().next().map(|r| SSAValue::from(*r)),
            _ => None,
        });

        assert!(result.is_demanded(if_result), "ret demands the if result");
        assert!(result.is_demanded(cond), "condition is a control root");
        assert!(
            result.is_demanded(constant_result(&pipeline, cfg, 1)),
            "then-arm yield operand feeds the demanded result"
        );
        assert!(
            result.is_demanded(constant_result(&pipeline, cfg, 2)),
            "else-arm yield operand feeds the demanded result"
        );
        assert!(
            !result.is_demanded(constant_result(&pipeline, cfg, 9)),
            "a dead constant inside a body stays dead"
        );
    }

    pub(super) const IF_DEAD_RESULT: &str = r#"
stage @source fn @if_dead(i64) -> i64;

specialize @source fn @if_dead(i64) -> i64 {
  ^entry(%cond: i64) {
    %result = if %cond then ^then() {
      %a = constant 1 -> i64;
      yield %a;
    } else ^else() {
      %b = constant 2 -> i64;
      yield %b;
    } -> i64;
    %zero = constant 0 -> i64;
    ret %zero;
  }
}
"#;

    /// A dead-result `scf.if` keeps only its condition live (the condition is
    /// an unconditional control root, consistent with `cf.cond_br`); the
    /// yield operands and body interiors stay dead.
    #[test]
    fn scf_if_dead_result_keeps_only_condition() {
        let pipeline = parse(IF_DEAD_RESULT);
        let (stage, cfg) = source_cfg(&pipeline, "if_dead");
        let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

        let cond = entry_params(&pipeline, cfg)[0];
        let if_result = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Structured(_) => definition.results().next().map(|r| SSAValue::from(*r)),
            _ => None,
        });

        assert!(result.is_demanded(cond), "condition is a control root");
        assert!(result.is_demanded(constant_result(&pipeline, cfg, 0)));
        assert!(!result.is_demanded(if_result));
        assert!(!result.is_demanded(constant_result(&pipeline, cfg, 1)));
        assert!(!result.is_demanded(constant_result(&pipeline, cfg, 2)));
    }

    pub(super) const FOR_CARRIED_DEMAND: &str = r#"
stage @source fn @loop_sum(i64, i64, i64) -> i64;

specialize @source fn @loop_sum(i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64) {
    %init = constant 0 -> i64;
    %sum = for %lo in %lo..%hi step %s iter_args(%init) do ^body(%i: i64, %acc: i64) {
      %one = constant 1 -> i64;
      %next = add %acc, %one -> i64;
      yield %next;
    } -> i64;
    ret %sum;
  }
}
"#;

    /// Loop-carried demand converges through the value worklist: the demanded
    /// result demands the yield slot, whose `add` demands the carried
    /// parameter, which re-runs the `for` rule and demands the initial value
    /// and the yield slot again (idempotent). The unused induction variable
    /// stays dead even though the loop runs.
    #[test]
    fn scf_for_loop_carried_demand_converges() {
        let pipeline = parse(FOR_CARRIED_DEMAND);
        let (stage, cfg) = source_cfg(&pipeline, "loop_sum");
        let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

        let params = entry_params(&pipeline, cfg);
        let (lo, hi, step) = (params[0], params[1], params[2]);
        let next = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Arith(Arith::Add { result, .. }) => Some(SSAValue::from(*result)),
            _ => None,
        });
        // Body params: [induction var, carried].
        let (induction, carried) = {
            let stage_id = pipeline.stage_by_name("source").expect("source stage");
            let Stage::Source(info) = pipeline.stage(stage_id).expect("stage info") else {
                panic!("source stage holds HighLevel");
            };
            let body = find_value(&pipeline, cfg, |definition| match definition {
                HighLevel::Structured(kirin_scf::StructuredControlFlow::For(op)) => Some(op.body()),
                _ => None,
            });
            let params: Vec<SSAValue> = body
                .expect_info(info)
                .arguments
                .iter()
                .copied()
                .map(SSAValue::from)
                .collect();
            (params[0], params[1])
        };

        assert!(
            result.is_demanded(constant_result(&pipeline, cfg, 0)),
            "init"
        );
        assert!(
            result.is_demanded(constant_result(&pipeline, cfg, 1)),
            "one"
        );
        assert!(result.is_demanded(next), "yield slot");
        assert!(result.is_demanded(carried), "carried param feeds the add");
        assert!(
            !result.is_demanded(induction),
            "the unused induction variable stays dead"
        );
        assert!(result.is_demanded(lo) && result.is_demanded(hi) && result.is_demanded(step));
    }

    const FOR_DEAD_RESULT: &str = r#"
stage @source fn @loop_dead(i64, i64, i64) -> i64;

specialize @source fn @loop_dead(i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64) {
    %init = constant 0 -> i64;
    %sum = for %lo in %lo..%hi step %s iter_args(%init) do ^body(%i: i64, %acc: i64) {
      %one = constant 1 -> i64;
      %next = add %acc, %one -> i64;
      yield %next;
    } -> i64;
    %seven = constant 7 -> i64;
    ret %seven;
  }
}
"#;

    /// A dead-result loop keeps only its bounds live: the carried chain
    /// (init, yield slot, body interior) stays dead.
    #[test]
    fn scf_for_dead_result_keeps_only_bounds() {
        let pipeline = parse(FOR_DEAD_RESULT);
        let (stage, cfg) = source_cfg(&pipeline, "loop_dead");
        let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

        let params = entry_params(&pipeline, cfg);
        let next = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Arith(Arith::Add { result, .. }) => Some(SSAValue::from(*result)),
            _ => None,
        });

        assert!(result.is_demanded(params[0]), "bounds are control roots");
        assert!(result.is_demanded(params[1]));
        assert!(result.is_demanded(params[2]));
        assert!(result.is_demanded(constant_result(&pipeline, cfg, 7)));
        assert!(
            !result.is_demanded(constant_result(&pipeline, cfg, 0)),
            "init dead"
        );
        assert!(
            !result.is_demanded(constant_result(&pipeline, cfg, 1)),
            "body interior dead"
        );
        assert!(!result.is_demanded(next), "yield slot dead");
    }

    const CALL_PURITY: &str = r#"
stage @source fn @callee(i64) -> i64;
stage @source fn @main(i64, i64) -> i64;

specialize @source fn @callee(i64) -> i64 {
  ^entry(%v: i64) {
    ret %v;
  }
}

specialize @source fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %y: i64) {
    %unused = call.named @callee(%x) -> i64;
    %deadsum = add %y, %y -> i64;
    %zero = constant 0 -> i64;
    ret %zero;
  }
}
"#;

    /// Calls are unmarked (impure), so their arguments are unconditional
    /// demand roots even when the call result is unused; a pure `add` beside
    /// it with a dead result demands nothing.
    #[test]
    fn call_arguments_are_demand_roots() {
        let pipeline = parse(CALL_PURITY);
        let (stage, cfg) = source_cfg(&pipeline, "main");
        let result = analyze_demand(&pipeline, stage, cfg).expect("analysis succeeds");

        let params = entry_params(&pipeline, cfg);
        let (x, y) = (params[0], params[1]);
        let unused = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Lexical(Lexical::Call(_)) => {
                definition.results().next().map(|r| SSAValue::from(*r))
            }
            _ => None,
        });
        let deadsum = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Arith(Arith::Add { result, .. }) => Some(SSAValue::from(*result)),
            _ => None,
        });

        assert!(result.is_demanded(x), "call args are roots (impure)");
        assert!(result.is_demanded(constant_result(&pipeline, cfg, 0)));
        assert!(!result.is_demanded(unused), "the call result is unused");
        assert!(!result.is_demanded(y), "pure add with dead result");
        assert!(!result.is_demanded(deadsum));
    }
}

// ===========================================================================
// Classic (dense, per-point) liveness through the toy language's total dense
// frame: arm-join for `scf.if`, the loop-carried fixpoint for `scf.for`, and
// per-point reconstruction inside structured bodies. Runs on the finalized IR
// alone — no demand pre-pass.
// ===========================================================================

mod dense {
    use kirin::prelude::{CFG, CompileStage, Pipeline, SSAValue};
    use kirin_arith::{Arith, ArithValue};
    use kirin_interpreter::{Body, InterpreterError, ProgramPoint, Scoped};
    use kirin_liveness::{DenseLivenessResult, LiveSet};
    use kirin_scf::StructuredControlFlow;

    use super::demand::{FOR_CARRIED_DEMAND, IF_DEAD_RESULT};
    use super::demand::{
        constant_result, entry_params, find_statement, find_value, parse, source_cfg,
    };
    use crate::interpreter::ToyDenseBackwardFrame;
    use crate::language::HighLevel;
    use crate::stage::Stage;

    /// Run classic dense liveness with the toy total frame.
    fn analyze_dense_toy(
        pipeline: &Pipeline<Stage>,
        stage: CompileStage,
        cfg: CFG,
    ) -> DenseLivenessResult {
        kirin_liveness::analyze_dense_with_frame::<
            _,
            ToyDenseBackwardFrame<LiveSet, InterpreterError>,
        >(pipeline, stage, cfg)
        .expect("analysis succeeds")
    }

    fn live_set(values: &[SSAValue]) -> LiveSet {
        values.iter().copied().collect()
    }

    /// Per-point sets inside an `scf.if` arm follow classic semantics: the
    /// yield's operand is live after its def even though the result is dead.
    /// (The strong view is the `dense ∩ demanded` composition, covered in
    /// kirin-liveness — no demand pass runs here.)
    #[test]
    fn dense_per_point_inside_scf_if_arm() {
        let pipeline = parse(IF_DEAD_RESULT);
        let (stage, cfg) = source_cfg(&pipeline, "if_dead");
        let dense = analyze_dense_toy(&pipeline, stage, cfg);
        let scope = (stage, Body::CFG(cfg));
        let point = |item| Scoped::new(scope, item);

        let cond = entry_params(&pipeline, cfg)[0];
        let a = constant_result(&pipeline, cfg, 1);
        let a_const = find_statement(&pipeline, cfg, |definition| match definition {
            HighLevel::Constant(constant) => constant.value == ArithValue::I64(1),
            _ => None::<()>.is_some(),
        });

        // Classic: after `%a = constant 1`, %a is live (the yield uses it)
        // and %cond flows through the arm; before it, %a is killed.
        assert_eq!(
            dense.point_facts(point(ProgramPoint::After(a_const))),
            Some(&live_set(&[cond, a]))
        );
        assert_eq!(
            dense.point_facts(point(ProgramPoint::Before(a_const))),
            Some(&live_set(&[cond]))
        );

        // The if's dead result is not live after it because nothing uses it;
        // before it, only the condition survives the arm join.
        let if_stmt = find_statement(&pipeline, cfg, |definition| {
            matches!(definition, HighLevel::Structured(_))
        });
        assert_eq!(
            dense.point_facts(point(ProgramPoint::Before(if_stmt))),
            Some(&live_set(&[cond]))
        );

        let then_block = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Structured(StructuredControlFlow::If(if_op)) => Some(if_op.then_block()),
            _ => None,
        });
        assert_eq!(
            dense.point_facts(point(ProgramPoint::BlockEntry(then_block))),
            Some(&live_set(&[cond]))
        );
        assert_eq!(
            dense.point_facts(point(ProgramPoint::BlockExit(then_block))),
            Some(&live_set(&[cond]))
        );
    }

    const IF_ARMS_DIFFERENT_USES: &str = r#"
stage @source fn @if_arms(i64, i64, i64) -> i64;

specialize @source fn @if_arms(i64, i64, i64) -> i64 {
  ^entry(%cond: i64, %x: i64, %y: i64) {
    %r = if %cond then ^then() {
      yield %x;
    } else ^else() {
      yield %y;
    } -> i64;
    ret %r;
  }
}
"#;

    /// The scf.if liveness frame walks BOTH arms backward and joins their
    /// live-entry states: the arms use different SSA values (%x vs %y), so
    /// the state before the `if` must contain the condition and both.
    #[test]
    fn dense_scf_if_joins_both_arm_entries() {
        let pipeline = parse(IF_ARMS_DIFFERENT_USES);
        let (stage, cfg) = source_cfg(&pipeline, "if_arms");
        let dense = analyze_dense_toy(&pipeline, stage, cfg);
        let scope = (stage, Body::CFG(cfg));

        let params = entry_params(&pipeline, cfg);
        let (cond, x, y) = (params[0], params[1], params[2]);
        let if_stmt = find_statement(&pipeline, cfg, |definition| {
            matches!(definition, HighLevel::Structured(_))
        });
        let r = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Structured(_) => {
                use kirin::prelude::HasResults;
                definition.results().next().map(|v| SSAValue::from(*v))
            }
            _ => None,
        });

        // After the if only its result matters; before it, the then-arm
        // contributed %x, the else-arm %y, and the rule genned %cond.
        assert_eq!(
            dense.point_facts(Scoped::new(scope, ProgramPoint::After(if_stmt))),
            Some(&live_set(&[r]))
        );
        assert_eq!(
            dense.point_facts(Scoped::new(scope, ProgramPoint::Before(if_stmt))),
            Some(&live_set(&[cond, x, y]))
        );
    }

    /// The scf.for dense frame iterates the body walk to the loop-carried
    /// fixpoint: the yield slot is live after the add (carried into the next
    /// iteration), the carried parameter is live before it, and the state
    /// before the loop is exactly bounds + initial values.
    #[test]
    fn dense_loop_carried_fixpoint() {
        let pipeline = parse(FOR_CARRIED_DEMAND);
        let (stage, cfg) = source_cfg(&pipeline, "loop_sum");
        let dense = analyze_dense_toy(&pipeline, stage, cfg);
        let scope = (stage, Body::CFG(cfg));

        let params = entry_params(&pipeline, cfg);
        let (lo, hi, step) = (params[0], params[1], params[2]);
        let init = constant_result(&pipeline, cfg, 0);
        let one = constant_result(&pipeline, cfg, 1);
        let (next, acc) = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Arith(Arith::Add { lhs, result, .. }) => {
                Some((SSAValue::from(*result), *lhs))
            }
            _ => None,
        });
        let for_stmt = find_statement(&pipeline, cfg, |definition| {
            matches!(definition, HighLevel::Structured(_))
        });
        let add_stmt = find_statement(&pipeline, cfg, |definition| {
            matches!(definition, HighLevel::Arith(Arith::Add { .. }))
        });
        let sum = find_value(&pipeline, cfg, |definition| match definition {
            HighLevel::Structured(_) => {
                use kirin::prelude::HasResults;
                definition.results().next().map(|r| SSAValue::from(*r))
            }
            _ => None,
        });

        // Around the loop.
        assert_eq!(
            dense.point_facts(Scoped::new(scope, ProgramPoint::After(for_stmt))),
            Some(&live_set(&[sum]))
        );
        assert_eq!(
            dense.point_facts(Scoped::new(scope, ProgramPoint::Before(for_stmt))),
            Some(&live_set(&[lo, hi, step, init]))
        );

        // Inside the body, at the stable iteration: the carried param and the
        // constant are live before the add; the yield slot is live after it
        // (it feeds the next iteration through the carry).
        assert_eq!(
            dense.point_facts(Scoped::new(scope, ProgramPoint::Before(add_stmt))),
            Some(&live_set(&[lo, hi, step, init, acc, one]))
        );
        assert_eq!(
            dense.point_facts(Scoped::new(scope, ProgramPoint::After(add_stmt))),
            Some(&live_set(&[lo, hi, step, init, next]))
        );
    }
}
