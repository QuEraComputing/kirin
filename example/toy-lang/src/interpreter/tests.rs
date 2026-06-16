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
        let region = builder.region().add_block(block).new();
        let body = Function::<ArithType>::new(
            builder,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        );
        builder
            .specialize()
            .staged_func(caller)
            .body(body)
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

/// Compiler/analysis-author surface: a *custom total frame enum* used
/// as the engine's `F` parameter (frame generalization), and a *custom abstract
/// policy* budget (summary-key generalization). Both reuse the standard engine
/// — no fork.
mod advanced {
    use std::cell::RefCell;

    use kirin_constprop::{ConstPropContext, ConstPropValue};
    use kirin_interpreter::engine::{
        AbstractInterpreter, CallFrame, Completion, ConcreteInterpreter, CrossStageLinker, Frame,
        FrameBuild, FrameDriver, FrameEffect, InterpreterError, SameStageLinker, ScopeFrame,
        expect_single,
    };

    use super::build_pipeline;
    use crate::interpreter::ToyError;
    use crate::stage::Stage;

    // --- A custom total frame enum -----------------------------------------
    //
    // It reuses the standard `ScopeFrame`/`CallFrame` traversal verbatim (via
    // `FrameBuild` + the delegating `*_into` methods) and adds *observation*:
    // every call and every scope step is counted in a side log. The engine is
    // not forked — only `ConcreteInterpreter`'s `F` type parameter changes.

    thread_local! {
        static TRACE: RefCell<Trace> = const { RefCell::new(Trace { calls: 0, scope_steps: 0 }) };
    }

    #[derive(Clone, Copy, Default)]
    struct Trace {
        calls: usize,
        scope_steps: usize,
    }

    enum TracingFrame<V, E> {
        Scope(ScopeFrame<V, E>),
        Call(CallFrame<V>),
    }

    impl<V, E> FrameBuild<V, E> for TracingFrame<V, E> {
        fn from_scope(frame: ScopeFrame<V, E>) -> Self {
            TracingFrame::Scope(frame)
        }
        fn from_call(frame: CallFrame<V>) -> Self {
            TracingFrame::Call(frame)
        }
    }

    impl<I> Frame<I> for TracingFrame<I::Value, I::Error>
    where
        I: FrameDriver,
        I::Value: Clone,
        I::Error: From<InterpreterError>,
    {
        type Completion = Completion<I::Value>;

        fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingFrame::Scope(frame) => {
                    TRACE.with(|t| t.borrow_mut().scope_steps += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingFrame::Call(frame) => {
                    TRACE.with(|t| t.borrow_mut().calls += 1);
                    frame.step_into::<I, Self>(interp)
                }
            }
        }

        fn resume_done(
            self,
            _interp: &mut I,
        ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingFrame::Scope(frame) => Ok(frame.resume_done_into::<Self>()),
                TracingFrame::Call(frame) => {
                    frame.resume_done_into::<Self>().map_err(I::Error::from)
                }
            }
        }

        fn resume(
            self,
            completion: Self::Completion,
            interp: &mut I,
        ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingFrame::Scope(frame) => frame.resume_into::<I, Self>(completion, interp),
                TracingFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
            }
        }
    }

    type TracingInterpreter<'ir> = ConcreteInterpreter<
        'ir,
        Stage,
        i64,
        ToyError,
        CrossStageLinker,
        TracingFrame<i64, ToyError>,
    >;

    #[test]
    fn custom_frame_runs_program_and_observes_traversal() {
        TRACE.with(|t| *t.borrow_mut() = Trace::default());
        let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));

        // ConcreteInterpreter parameterized by the *custom* frame enum.
        let mut interp: TracingInterpreter<'_> =
            ConcreteInterpreter::new(&pipeline).with_linker(CrossStageLinker);
        let result = expect_single::<i64, ToyError>(
            interp.call_by_name("source", "factorial", [5]).unwrap(),
        )
        .unwrap();

        // (1)+(2): the custom frame ran the real program correctly by reusing
        // the standard ScopeFrame/CallFrame traversal (no engine fork).
        assert_eq!(result, 120);

        // (3): traversal is observable through the custom frame. factorial(5)
        // makes 4 recursive calls (5→4→3→2→1; the base case at 1 makes none),
        // all routed through the custom Call arm; body statements run through
        // its Scope arm.
        let trace = TRACE.with(|t| *t.borrow());
        assert_eq!(trace.calls, 4);
        assert!(trace.scope_steps > 0);
    }

    // --- A capped custom abstract policy -----------------------------------

    #[test]
    fn constprop_context_budget_overflow_falls_back_to_top() {
        // Budget 2 admits the [5] and [4] contexts; [3]/[2]/[1] overflow to the
        // shared `Unknown` context (joined → Top). The result is soundly `Top`
        // (capping degrades precision, not soundness) — and it terminates,
        // which exercises the same-key self-dependency convergence.
        let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));
        let base: AbstractInterpreter<
            '_,
            Stage,
            ConstPropValue,
            ToyError,
            SameStageLinker,
            ConstPropContext,
        > = AbstractInterpreter::new(&pipeline);
        let mut analysis = base
            .with_analysis(ConstPropContext::with_budget(2))
            .with_linker(CrossStageLinker);
        let result = expect_single::<ConstPropValue, ToyError>(
            analysis
                .analyze_by_name("source", "factorial", [ConstPropValue::Const(5)])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result, ConstPropValue::Top);
    }
}
