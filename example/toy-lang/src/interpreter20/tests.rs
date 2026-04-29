use std::collections::{HashMap, HashSet};

use kirin::prelude::*;
use kirin_interpreter::AbstractValue;
use kirin_interpreter_20::abstract_interp::AbstractInterp;
use kirin_interpreter_20::backward::{BackwardFixpoint, BlockTransferBackward};
use kirin_interpreter_20::concrete::ConcreteInterp;
use kirin_interval::Interval;

use crate::interpreter20::cursors::{
    AbstractMultiCursor, HighLevelAbstractCursor, HighLevelCursor, LowLevelAbstract,
};
use crate::interpreter20::domains::{ConstProp, ToyType};
use crate::interpreter20::interp::{AbstractMultiInterp, MultiInterp, ToyVal};
use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use kirin_interpreter_20::execute::Execute;
use kirin_interpreter_20::interpretable::Interpretable;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn build_pipeline(src: &str) -> Pipeline<Stage> {
    let mut p = Pipeline::new();
    ParsePipelineText::parse(&mut p, src).expect("parse failed");
    p
}

fn run_concrete_i64_highlevel(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
    let pipeline = build_pipeline(src);
    run_concrete_i64_highlevel_on(&pipeline, func_name, args)
}

fn run_concrete_i64_highlevel_on<'ir>(
    pipeline: &'ir Pipeline<Stage>,
    func_name: &str,
    args: &[i64],
) -> Option<i64> {
    let stage_id = pipeline.stage_by_name("source").unwrap();
    let stage_info: &StageInfo<HighLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function(func_name, stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let mut interp: ConcreteInterp<'ir, Stage, HighLevel, i64, HighLevelCursor<i64>> =
        ConcreteInterp::new(pipeline, stage_id);
    interp.run_function::<HighLevel>(spec, args).unwrap()
}

fn analyze_lowered<'ir, V>(
    pipeline: &'ir Pipeline<Stage>,
    func_name: &str,
    args: Vec<V>,
) -> Option<V>
where
    V: ToyVal + AbstractValue,
    LowLevel: Interpretable<AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstract<V>>>,
    LowLevelAbstract<V>: Execute<AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstract<V>>>,
{
    let stage_id = pipeline.stage_by_name("lowered").unwrap();
    let stage_info: &StageInfo<LowLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function(func_name, stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let mut interp: AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstract<V>> =
        AbstractInterp::new(pipeline, stage_id);
    interp
        .analyze(spec, stage_id, args)
        .expect("analysis failed")
}

fn analyze_highlevel<'ir, V>(
    pipeline: &'ir Pipeline<Stage>,
    func_name: &str,
    args: Vec<V>,
) -> Option<V>
where
    V: ToyVal + AbstractValue,
    HighLevel: Interpretable<AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>,
    HighLevelAbstractCursor<V>:
        Execute<AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>,
{
    let stage_id = pipeline.stage_by_name("source").unwrap();
    let stage_info: &StageInfo<HighLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function(func_name, stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let mut interp: AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>> =
        AbstractInterp::new(pipeline, stage_id);
    interp
        .analyze(spec, stage_id, args)
        .expect("analysis failed")
}

fn analyze_multi<'ir, V>(pipeline: &'ir Pipeline<Stage>, func_name: &str, args: Vec<V>) -> Option<V>
where
    V: ToyVal + AbstractValue,
    HighLevel: Interpretable<AbstractMultiInterp<'ir, V>>,
    LowLevel: Interpretable<AbstractMultiInterp<'ir, V>>,
    AbstractMultiCursor<V>: Execute<AbstractMultiInterp<'ir, V>>,
{
    let stage_id = pipeline.stage_by_name("source").unwrap();
    let stage_info: &StageInfo<HighLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function(func_name, stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let mut interp: AbstractMultiInterp<'ir, V> = AbstractMultiInterp::new(pipeline, stage_id);
    interp
        .analyze(spec, stage_id, args)
        .expect("analysis failed")
}

fn run_multi_i64(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
    let pipeline = build_pipeline(src);
    run_multi_i64_on(&pipeline, func_name, args)
}

fn run_multi_i64_on<'ir>(
    pipeline: &'ir Pipeline<Stage>,
    func_name: &str,
    args: &[i64],
) -> Option<i64> {
    let stage_id = pipeline.stage_by_name("source").unwrap();
    let stage_info: &StageInfo<HighLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function(func_name, stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
    interp.run_function::<HighLevel>(spec, args).unwrap()
}

fn run_multi_from_stage<'ir>(
    pipeline: &'ir Pipeline<Stage>,
    stage_name: &str,
    func_name: &str,
    args: &[i64],
) -> Option<i64> {
    let stage_id = pipeline.stage_by_name(stage_name).unwrap();
    match pipeline.stage(stage_id).unwrap() {
        Stage::Source(stage_info) => {
            let spec = pipeline
                .resolve_staged_function(func_name, stage_id)
                .unwrap()
                .get_info(stage_info)
                .unwrap()
                .unique_live_specialization()
                .unwrap();
            let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
            interp.run_function::<HighLevel>(spec, args).unwrap()
        }
        Stage::Lowered(stage_info) => {
            let spec = pipeline
                .resolve_staged_function(func_name, stage_id)
                .unwrap()
                .get_info(stage_info)
                .unwrap()
                .unique_live_specialization()
                .unwrap();
            let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
            interp.run_function::<LowLevel>(spec, args).unwrap()
        }
    }
}

// ---------------------------------------------------------------------------
// Source programs (HighLevel / SCF)
// ---------------------------------------------------------------------------

const ADD_SOURCE: &str = r#"
stage @source fn @add(i64, i64) -> i64;

specialize @source fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

const FACTORIAL_SOURCE: &str = r#"
stage @source fn @factorial(i64) -> i64;

specialize @source fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    %result = if %is_base then ^base() {
      yield %one;
    } else ^recurse() {
      %n_minus_1 = sub %n, %one -> i64;
      %rec = call @factorial(%n_minus_1) -> i64;
      %prod = mul %n, %rec -> i64;
      yield %prod;
    } -> i64;
    ret %result;
  }
}
"#;

const ABS_SOURCE: &str = r#"
stage @source fn @abs(i64) -> i64;

specialize @source fn @abs(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    %result = if %is_neg then ^neg() {
      %neg_x = neg %x -> i64;
      yield %neg_x;
    } else ^pos() {
      yield %x;
    } -> i64;
    ret %result;
  }
}
"#;

const FOR_SUM_SOURCE: &str = r#"
stage @source fn @sum_range(i64) -> i64;

specialize @source fn @sum_range(i64) -> i64 {
  ^entry(%n: i64) {
    %zero = constant 0 -> i64;
    %one = constant 1 -> i64;
    %result = for %zero in %zero..%n step %one iter_args(%zero) do ^body(%i: i64, %acc: i64) {
      %new_acc = add %acc, %i -> i64;
      yield %new_acc;
    } -> i64;
    ret %result;
  }
}
"#;

// ---------------------------------------------------------------------------
// Lowered programs (flat CF)
// ---------------------------------------------------------------------------

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

const FACTORIAL_LOWERED: &str = r#"
stage @lowered fn @factorial(i64) -> i64;

specialize @lowered fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    cond_br %is_base then=^base() else=^recurse();
  }
  ^base() {
    %one2 = constant 1 -> i64;
    ret %one2;
  }
  ^recurse() {
    %one3 = constant 1 -> i64;
    %n_minus_1 = sub %n, %one3 -> i64;
    %rec = call @factorial(%n_minus_1) -> i64;
    %prod = mul %n, %rec -> i64;
    ret %prod;
  }
}
"#;

// ---------------------------------------------------------------------------
// Cross-stage programs
// ---------------------------------------------------------------------------

const CROSS_STAGE_SRC: &str = r#"
stage @source fn @main(i64) -> i64;
stage @lowered fn @double(i64) -> i64;

specialize @source fn @main(i64) -> i64 {
  ^entry(%n: i64) {
    %result = call @double(%n) -> i64;
    ret %result;
  }
}

specialize @lowered fn @double(i64) -> i64 {
  ^entry(%n: i64) {
    %r = add %n, %n -> i64;
    ret %r;
  }
}
"#;

const SAME_STAGE_CALL_SRC: &str = r#"
stage @source fn @add(i64, i64) -> i64;
stage @source fn @wrapper(i64, i64) -> i64;

specialize @source fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %r = add %a, %b -> i64;
    ret %r;
  }
}

specialize @source fn @wrapper(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %r = call @add(%a, %b) -> i64;
    ret %r;
  }
}
"#;

// ---------------------------------------------------------------------------
// R9: Entry flexibility programs
// ---------------------------------------------------------------------------

const LOWERED_CALLS_SOURCE_SRC: &str = r#"
stage @source fn @square(i64) -> i64;
stage @lowered fn @lowered_main(i64) -> i64;

specialize @source fn @square(i64) -> i64 {
  ^entry(%n: i64) {
    %r = mul %n, %n -> i64;
    ret %r;
  }
}

specialize @lowered fn @lowered_main(i64) -> i64 {
  ^entry(%n: i64) {
    %r = call @square(%n) -> i64;
    ret %r;
  }
}
"#;

const SYMMETRIC_SRC: &str = r#"
stage @source fn @double(i64) -> i64;
stage @lowered fn @double(i64) -> i64;

specialize @source fn @double(i64) -> i64 {
  ^entry(%n: i64) {
    %r = add %n, %n -> i64;
    ret %r;
  }
}

specialize @lowered fn @double(i64) -> i64 {
  ^entry(%n: i64) {
    %r = add %n, %n -> i64;
    ret %r;
  }
}
"#;

// 3-arg function where %c is unused — for sparse AI tests
const SPARSE_PROG: &str = r#"
stage @lowered fn @maybe_add(i64, i64, i64) -> i64;

specialize @lowered fn @maybe_add(i64, i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64, %c: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

// ---------------------------------------------------------------------------
// Concrete tests (HighLevel / source stage, SCF)
// ---------------------------------------------------------------------------

#[test]
fn test_add_highlevel() {
    let result = run_concrete_i64_highlevel(ADD_SOURCE, "add", &[3i64, 5i64]);
    assert_eq!(result, Some(8));
}

#[test]
fn test_factorial() {
    let result = run_concrete_i64_highlevel(FACTORIAL_SOURCE, "factorial", &[5i64]);
    assert_eq!(result, Some(120));
}

#[test]
fn test_abs_positive() {
    let result = run_concrete_i64_highlevel(ABS_SOURCE, "abs", &[42i64]);
    assert_eq!(result, Some(42));
}

#[test]
fn test_abs_negative() {
    let result = run_concrete_i64_highlevel(ABS_SOURCE, "abs", &[-7i64]);
    assert_eq!(result, Some(7));
}

#[test]
fn for_loop_sum_concrete() {
    let result = run_concrete_i64_highlevel(FOR_SUM_SOURCE, "sum_range", &[5i64]);
    assert_eq!(result, Some(10));
}

#[test]
fn for_loop_sum_zero_iterations() {
    let result = run_concrete_i64_highlevel(FOR_SUM_SOURCE, "sum_range", &[0i64]);
    assert_eq!(result, Some(0));
}

#[test]
fn for_loop_abstract_converges() {
    let pipeline = build_pipeline(FOR_SUM_SOURCE);
    let result = analyze_highlevel::<ToyType>(&pipeline, "sum_range", vec![ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

// ---------------------------------------------------------------------------
// Abstract tests (LowLevel, Interval domain)
// ---------------------------------------------------------------------------

#[test]
fn interval_add_known_range() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = analyze_lowered::<Interval>(
        &pipeline,
        "add",
        vec![Interval::new(1, 3), Interval::new(2, 4)],
    );
    assert_eq!(result, Some(Interval::new(3, 7)));
}

#[test]
fn interval_branch_joins_both_paths() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_lowered::<Interval>(&pipeline, "sign", vec![Interval::new(-5, 5)]);
    assert_eq!(result, Some(Interval::new(0, 1)));
}

#[test]
fn interval_factorial_converges() {
    let pipeline = build_pipeline(FACTORIAL_LOWERED);
    let result = analyze_lowered::<Interval>(&pipeline, "factorial", vec![Interval::new(0, 10)]);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(!r.is_empty());
}

// ---------------------------------------------------------------------------
// Abstract tests (HighLevel, ToyType)
// ---------------------------------------------------------------------------

#[test]
fn toytype_add_highlevel_abstract() {
    let pipeline = build_pipeline(ADD_SOURCE);
    let result = analyze_highlevel::<ToyType>(&pipeline, "add", vec![ToyType::I64, ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

#[test]
fn toytype_abs_highlevel_abstract() {
    let pipeline = build_pipeline(ABS_SOURCE);
    let result = analyze_highlevel::<ToyType>(&pipeline, "abs", vec![ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

#[test]
fn toytype_factorial_highlevel_abstract() {
    let pipeline = build_pipeline(FACTORIAL_SOURCE);
    let result = analyze_highlevel::<ToyType>(&pipeline, "factorial", vec![ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

#[test]
fn toytype_lowered_add_propagates_i64() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = analyze_lowered::<ToyType>(&pipeline, "add", vec![ToyType::I64, ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

// ---------------------------------------------------------------------------
// Multi-stage concrete interpreter tests
// ---------------------------------------------------------------------------

#[test]
fn multi_cross_stage_source_calls_lowered() {
    let result = run_multi_i64(CROSS_STAGE_SRC, "main", &[7i64]);
    assert_eq!(result, Some(14));
}

#[test]
fn multi_cross_stage_double_five() {
    let result = run_multi_i64(CROSS_STAGE_SRC, "main", &[5i64]);
    assert_eq!(result, Some(10));
}

#[test]
fn multi_same_stage_call_through_dispatch() {
    let result = run_multi_i64(SAME_STAGE_CALL_SRC, "wrapper", &[3i64, 4i64]);
    assert_eq!(result, Some(7));
}

// ---------------------------------------------------------------------------
// Multi-stage abstract interpreter tests
// ---------------------------------------------------------------------------

#[test]
fn abstract_multi_same_stage_type_propagates() {
    let pipeline = build_pipeline(SAME_STAGE_CALL_SRC);
    let result = analyze_multi::<ToyType>(&pipeline, "wrapper", vec![ToyType::I64, ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

#[test]
fn abstract_multi_cross_stage_type_propagates() {
    let pipeline = build_pipeline(CROSS_STAGE_SRC);
    let result = analyze_multi::<ToyType>(&pipeline, "main", vec![ToyType::I64]);
    assert_eq!(result, Some(ToyType::I64));
}

#[test]
fn interval_cross_stage_doubles_range() {
    let pipeline = build_pipeline(CROSS_STAGE_SRC);
    let result = analyze_multi::<Interval>(&pipeline, "main", vec![Interval::new(1, 3)]);
    assert_eq!(result, Some(Interval::new(2, 6)));
}

// ---------------------------------------------------------------------------
// R9: Entry flexibility tests
// ---------------------------------------------------------------------------

#[test]
fn lowered_entry_calls_source() {
    let pipeline = build_pipeline(LOWERED_CALLS_SOURCE_SRC);
    let result = run_multi_from_stage(&pipeline, "lowered", "lowered_main", &[5i64]);
    assert_eq!(result, Some(25));
}

#[test]
fn symmetric_entry_highlevel() {
    let pipeline = build_pipeline(SYMMETRIC_SRC);
    let result = run_multi_from_stage(&pipeline, "source", "double", &[7i64]);
    assert_eq!(result, Some(14));
}

#[test]
fn symmetric_entry_lowlevel() {
    let pipeline = build_pipeline(SYMMETRIC_SRC);
    let result = run_multi_from_stage(&pipeline, "lowered", "double", &[7i64]);
    assert_eq!(result, Some(14));
}

// ---------------------------------------------------------------------------
// Backward liveness analysis (extensibility probe)
// ---------------------------------------------------------------------------

struct LivenessResult {
    live_in: HashMap<Block, HashSet<SSAValue>>,
    live_out: HashMap<Block, HashSet<SSAValue>>,
}

struct LivenessTransfer;

impl<'ir> BlockTransferBackward<'ir> for LivenessTransfer {
    type Domain = HashSet<SSAValue>;

    fn join(a: &HashSet<SSAValue>, b: &HashSet<SSAValue>) -> HashSet<SSAValue> {
        a.union(b).copied().collect()
    }

    fn bottom() -> HashSet<SSAValue> {
        HashSet::new()
    }

    fn transfer_block<L: Dialect>(
        &self,
        block: Block,
        stage: &StageInfo<L>,
        live_out: HashSet<SSAValue>,
    ) -> HashSet<SSAValue> {
        let info = block.expect_info(stage);
        let mut def_set: HashSet<SSAValue> = HashSet::new();
        let mut use_set: HashSet<SSAValue> = HashSet::new();

        for &ba in &info.arguments {
            def_set.insert(ba.into());
        }

        let mut process_stmt = |stmt: Statement| {
            for &val in stmt.arguments(stage) {
                if !def_set.contains(&val) {
                    use_set.insert(val);
                }
            }
            for &rv in stmt.results(stage) {
                def_set.insert(rv.into());
            }
        };

        for stmt in block.statements(stage) {
            process_stmt(stmt);
        }
        if let Some(term) = block.terminator(stage) {
            process_stmt(term);
        }

        use_set
            .into_iter()
            .chain(live_out.into_iter().filter(|v| !def_set.contains(v)))
            .collect()
    }
}

fn analyze_liveness<L: Dialect>(body_stmt: Statement, stage: &StageInfo<L>) -> LivenessResult {
    let fp = BackwardFixpoint::new(LivenessTransfer);
    let result = fp.analyze(body_stmt, stage);
    let mut live_in = HashMap::new();
    let mut live_out = HashMap::new();
    for (block, (li, lo)) in result {
        live_in.insert(block, li);
        live_out.insert(block, lo);
    }
    LivenessResult { live_in, live_out }
}

fn liveness_for_lowered_fn(pipeline: &Pipeline<Stage>, func_name: &str) -> LivenessResult {
    let stage_id = pipeline.stage_by_name("lowered").unwrap();
    let stage_info: &StageInfo<LowLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function(func_name, stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let body_stmt = *spec.get_info(stage_info).unwrap().body();
    analyze_liveness(body_stmt, stage_info)
}

fn collect_free_vars<L: Dialect>(block: Block, stage: &StageInfo<L>) -> HashSet<SSAValue> {
    let block_info = block.expect_info(stage);
    let mut local_defs: HashSet<SSAValue> = block_info
        .arguments
        .iter()
        .map(|ba| SSAValue::from(*ba))
        .collect();
    let mut free: HashSet<SSAValue> = HashSet::new();

    let process =
        |stmt: Statement, local_defs: &mut HashSet<SSAValue>, free: &mut HashSet<SSAValue>| {
            for &val in stmt.arguments(stage) {
                if !local_defs.contains(&val) {
                    free.insert(val);
                }
            }
            for nested in stmt.blocks(stage) {
                for val in collect_free_vars(*nested, stage) {
                    if !local_defs.contains(&val) {
                        free.insert(val);
                    }
                }
            }
            for &rv in stmt.results(stage) {
                local_defs.insert(rv.into());
            }
        };

    for stmt in block.statements(stage) {
        process(stmt, &mut local_defs, &mut free);
    }
    if let Some(term) = block.terminator(stage) {
        process(term, &mut local_defs, &mut free);
    }
    free
}

fn stmt_backward_liveness<L: Dialect>(
    top_block: Block,
    stage: &StageInfo<L>,
) -> HashMap<Statement, HashSet<SSAValue>> {
    let mut stmts: Vec<Statement> = top_block.statements(stage).collect();
    if let Some(term) = top_block.terminator(stage) {
        stmts.push(term);
    }

    let mut live: HashSet<SSAValue> = HashSet::new();
    let mut result: HashMap<Statement, HashSet<SSAValue>> = HashMap::new();

    for &stmt in stmts.iter().rev() {
        let mut uses: HashSet<SSAValue> = HashSet::new();
        for &val in stmt.arguments(stage) {
            uses.insert(val);
        }
        for nested in stmt.blocks(stage) {
            for val in collect_free_vars(*nested, stage) {
                uses.insert(val);
            }
        }
        let defs: HashSet<SSAValue> = stmt.results(stage).map(|rv| SSAValue::from(*rv)).collect();

        let live_before: HashSet<SSAValue> = uses
            .iter()
            .copied()
            .chain(live.iter().filter(|v| !defs.contains(v)).copied())
            .collect();

        result.insert(stmt, live_before.clone());
        live = live_before;
    }

    result
}

#[test]
fn liveness_add_args_live_at_entry() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = liveness_for_lowered_fn(&pipeline, "add");

    assert_eq!(result.live_in.len(), 1, "ADD_LOWERED should have 1 block");
    let (_, live_in) = result.live_in.iter().next().unwrap();
    assert!(
        live_in.is_empty(),
        "all values in ADD_LOWERED are locally defined; live_in must be empty"
    );
    let (_, live_out) = result.live_out.iter().next().unwrap();
    assert!(
        live_out.is_empty(),
        "single-exit block has no successors; live_out must be empty"
    );
}

#[test]
fn liveness_dead_after_use() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = liveness_for_lowered_fn(&pipeline, "sign");

    assert_eq!(
        result.live_in.len(),
        3,
        "BRANCH_LOWERED should have 3 blocks"
    );
    for li in result.live_in.values() {
        assert!(
            li.is_empty(),
            "no value crosses a block boundary; live_in must be empty"
        );
    }
    for lo in result.live_out.values() {
        assert!(
            lo.is_empty(),
            "no value crosses a block boundary; live_out must be empty"
        );
    }
}

#[test]
fn liveness_cross_block_use_in_factorial() {
    let pipeline = build_pipeline(FACTORIAL_LOWERED);
    let stage_id = pipeline.stage_by_name("lowered").unwrap();
    let stage_info: &StageInfo<LowLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function("factorial", stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let spec_info = spec.get_info(stage_info).unwrap();
    let body_stmt = *spec_info.body();
    let result = analyze_liveness(body_stmt, stage_info);

    let region = body_stmt.regions(stage_info).next().unwrap();
    let mut block_iter = region.blocks(stage_info);
    let entry_block = block_iter.next().expect("entry block must exist");
    let base_block = block_iter.next().expect("base block must exist");
    let recurse_block = block_iter.next().expect("recurse block must exist");

    let entry_info = entry_block.expect_info(stage_info);
    let n_ssa: SSAValue = entry_info.arguments[0].into();

    assert!(
        result.live_in[&recurse_block].contains(&n_ssa),
        "%n must be live-in of ^recurse"
    );
    assert!(
        result.live_out[&entry_block].contains(&n_ssa),
        "%n must be live-out of ^entry"
    );
    assert!(
        !result.live_in[&entry_block].contains(&n_ssa),
        "%n is defined by ^entry as block arg; not in live_in[^entry]"
    );
    assert!(
        !result.live_in[&base_block].contains(&n_ssa),
        "%n is not used in ^base; not in live_in[^base]"
    );
}

#[test]
fn backward_liveness_highlevel() {
    let pipeline = build_pipeline(ABS_SOURCE);
    let stage_id = pipeline.stage_by_name("source").unwrap();
    let stage_info: &StageInfo<HighLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function("abs", stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let spec_info = spec.get_info(stage_info).unwrap();
    let body_stmt = *spec_info.body();

    let region = body_stmt.regions(stage_info).next().unwrap();
    let top_block = region.blocks(stage_info).next().unwrap();

    let liveness = stmt_backward_liveness(top_block, stage_info);

    let stmts: Vec<Statement> = top_block.statements(stage_info).collect();
    assert!(
        stmts.len() >= 3,
        "ABS_SOURCE should have at least 3 non-terminator statements"
    );
    let if_stmt = stmts[2];

    let live_before_if = &liveness[&if_stmt];

    let block_info = top_block.expect_info(stage_info);
    let x_ssa: SSAValue = block_info.arguments[0].into();
    let is_neg_ssa: SSAValue = SSAValue::from(*stmts[1].results(stage_info).next().unwrap());

    assert!(
        live_before_if.contains(&x_ssa),
        "%x must be live before the scf.if"
    );
    assert!(
        live_before_if.contains(&is_neg_ssa),
        "%is_neg must be live before the scf.if"
    );
}

#[test]
fn backward_liveness_scf() {
    let pipeline = build_pipeline(FACTORIAL_SOURCE);
    let stage_id = pipeline.stage_by_name("source").unwrap();
    let stage_info: &StageInfo<HighLevel> =
        pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
    let spec = pipeline
        .resolve_staged_function("factorial", stage_id)
        .unwrap()
        .get_info(stage_info)
        .unwrap()
        .unique_live_specialization()
        .unwrap();
    let spec_info = spec.get_info(stage_info).unwrap();
    let body_stmt = *spec_info.body();

    let region = body_stmt.regions(stage_info).next().unwrap();
    let top_block = region.blocks(stage_info).next().unwrap();

    let liveness = stmt_backward_liveness(top_block, stage_info);

    let stmts: Vec<Statement> = top_block.statements(stage_info).collect();
    assert!(
        stmts.len() >= 3,
        "FACTORIAL_SOURCE should have at least 3 non-terminator statements"
    );
    let if_stmt = stmts[2];

    let live_before_if = &liveness[&if_stmt];

    let block_info = top_block.expect_info(stage_info);
    let n_ssa: SSAValue = block_info.arguments[0].into();
    let one_ssa: SSAValue = SSAValue::from(*stmts[0].results(stage_info).next().unwrap());
    let is_base_ssa: SSAValue = SSAValue::from(*stmts[1].results(stage_info).next().unwrap());

    assert!(
        live_before_if.contains(&n_ssa),
        "%n must be live before scf.if"
    );
    assert!(
        live_before_if.contains(&one_ssa),
        "%one must be live before scf.if"
    );
    assert!(
        live_before_if.contains(&is_base_ssa),
        "%is_base must be live before scf.if"
    );
}

// ---------------------------------------------------------------------------
// ConstProp extensibility probe (R8)
// ---------------------------------------------------------------------------

#[test]
fn constprop_add_two_constants() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result = analyze_lowered::<ConstProp>(
        &pipeline,
        "add",
        vec![ConstProp::Const(2), ConstProp::Const(3)],
    );
    assert_eq!(result, Some(ConstProp::Const(5)));
}

#[test]
fn constprop_top_input_propagates() {
    let pipeline = build_pipeline(ADD_LOWERED);
    let result =
        analyze_lowered::<ConstProp>(&pipeline, "add", vec![ConstProp::Top, ConstProp::Const(3)]);
    assert_eq!(result, Some(ConstProp::Top));
}

#[test]
fn constprop_branch_positive_input() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Const(5)]);
    assert_eq!(result, Some(ConstProp::Const(0)));
}

#[test]
fn constprop_branch_negative_input() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Const(-3)]);
    assert_eq!(result, Some(ConstProp::Const(1)));
}

#[test]
fn constprop_branch_unknown_joins_both_paths() {
    let pipeline = build_pipeline(BRANCH_LOWERED);
    let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Top]);
    assert_eq!(result, Some(ConstProp::Top));
}

// ---------------------------------------------------------------------------
// Sparse abstract interpretation tests
// ---------------------------------------------------------------------------

#[test]
fn sparse_interval_propagation() {
    let pipeline = build_pipeline(SPARSE_PROG);
    let result = analyze_lowered::<Interval>(
        &pipeline,
        "maybe_add",
        vec![Interval::new(1, 3), Interval::new(2, 4), Interval::bottom()],
    );
    assert_eq!(
        result,
        Some(Interval::new(3, 7)),
        "sparse AI: seeded args propagate; unused bottom arg ignored"
    );
}

#[test]
fn sparse_type_propagation() {
    let pipeline = build_pipeline(SPARSE_PROG);
    let result = analyze_lowered::<ToyType>(
        &pipeline,
        "maybe_add",
        vec![ToyType::I64, ToyType::I64, ToyType::Bottom],
    );
    assert_eq!(
        result,
        Some(ToyType::I64),
        "sparse AI: type propagates from seeded values; Bottom arg does not pollute result"
    );
}
