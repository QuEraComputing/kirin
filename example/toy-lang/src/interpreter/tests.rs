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

// `AbstractScfIfFrame` owns the "explore both arms + join" behavior that used to
// live in the removed `ForwardEffect::EnterAny` / framework alternatives frame.
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

/// Compiler/analysis-author surface: a *custom total frame enum* used
/// as the engine's `F` parameter (frame generalization), and a *custom abstract
/// policy* budget (summary-key generalization). Both reuse the standard engine
/// — no fork.
mod advanced {
    use std::cell::RefCell;
    use std::hash::Hash;

    use kirin_constprop::{ConstPropContext, ConstPropValue};
    use kirin_interpreter::engine::{
        AbstractBlockFrame, AbstractCallFrame, AbstractCfgFrame, AbstractCompletion,
        AbstractFrameBuild, AbstractFrameDriver, AbstractFunctionFrame, AbstractInterpreter,
        BodyFrame, CallContext, CallFrame, Completion, ConcreteInterpreter, CrossStageLinker,
        ForwardInterp, Frame, FrameBuild, FrameDriver, FrameEffect, InterpreterError,
        expect_single,
    };
    use kirin_scf::{
        AbstractScfForFrame, AbstractScfIfFrame, BuildAbstractScfFor, BuildAbstractScfIf,
        BuildScfFor, BuildScfIf, ForLoopValue, ScfForFrame, ScfIfFrame,
    };

    use super::build_pipeline;
    use crate::interpreter::ToyError;
    use crate::stage::Stage;

    // --- A custom total frame enum -----------------------------------------
    //
    // It reuses the standard `BodyFrame`/`CallFrame` traversal (and the SCF loop
    // frame) verbatim via `FrameBuild`/`BuildScfFor` + the delegating `*_into`
    // methods, and adds *observation*: every call and every body step is counted
    // in a side log. The engine is not forked — only `ConcreteInterpreter`'s `F`
    // type parameter changes.

    thread_local! {
        static TRACE: RefCell<Trace> = const { RefCell::new(Trace { calls: 0, body_steps: 0 }) };
    }

    #[derive(Clone, Copy, Default)]
    struct Trace {
        calls: usize,
        body_steps: usize,
    }

    enum TracingFrame<V, E> {
        Body(BodyFrame<V, E>),
        Call(CallFrame<V>),
        ScfIf(ScfIfFrame<V, E>),
        ScfFor(ScfForFrame<V, E>),
    }

    impl<V, E> FrameBuild<V, E> for TracingFrame<V, E> {
        fn from_body(frame: BodyFrame<V, E>) -> Self {
            TracingFrame::Body(frame)
        }
        fn from_call(frame: CallFrame<V>) -> Self {
            TracingFrame::Call(frame)
        }
    }

    impl<V, E> BuildScfIf<V, E> for TracingFrame<V, E> {
        fn scf_if(frame: ScfIfFrame<V, E>) -> Self {
            TracingFrame::ScfIf(frame)
        }
    }

    impl<V, E> BuildScfFor<V, E> for TracingFrame<V, E> {
        fn scf_for(frame: ScfForFrame<V, E>) -> Self {
            TracingFrame::ScfFor(frame)
        }
    }

    impl<I, V, E> Frame<I> for TracingFrame<V, E>
    where
        I: FrameDriver<Value = V, Error = E> + ForwardInterp<Frame = TracingFrame<V, E>>,
        V: Clone + ForLoopValue,
        E: From<InterpreterError>,
    {
        type Completion = Completion<V>;

        fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingFrame::Body(frame) => {
                    TRACE.with(|t| t.borrow_mut().body_steps += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingFrame::Call(frame) => {
                    TRACE.with(|t| t.borrow_mut().calls += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingFrame::ScfIf(frame) => frame.step_into::<I, Self>(interp),
                TracingFrame::ScfFor(frame) => frame.step_into::<I, Self>(interp),
            }
        }

        fn resume_done(
            self,
            _interp: &mut I,
        ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingFrame::Body(frame) => Ok(frame.resume_done_into::<Self>()),
                TracingFrame::Call(frame) => {
                    frame.resume_done_into::<Self>().map_err(I::Error::from)
                }
                TracingFrame::ScfIf(frame) => frame.resume_done_into::<Self>(),
                TracingFrame::ScfFor(frame) => frame.resume_done_into::<Self>(),
            }
        }

        fn resume(
            self,
            completion: Self::Completion,
            interp: &mut I,
        ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingFrame::Body(frame) => frame.resume_into::<I, Self>(completion, interp),
                TracingFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
                TracingFrame::ScfIf(frame) => frame.resume_into::<Self>(completion),
                TracingFrame::ScfFor(frame) => frame.resume_into::<I, Self>(completion, interp),
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
        // the standard BodyFrame/CallFrame traversal (no engine fork).
        assert_eq!(result, 120);

        // (3): traversal is observable through the custom frame. factorial(5)
        // makes 4 recursive calls (5→4→3→2→1; the base case at 1 makes none),
        // all routed through the custom Call arm; body statements run through
        // its Body arm.
        let trace = TRACE.with(|t| *t.borrow());
        assert_eq!(trace.calls, 4);
        assert!(trace.body_steps > 0);
    }

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
            .with_linker(CrossStageLinker);
        let result = expect_single::<ConstPropValue, ToyError>(
            analysis
                .analyze_by_name("source", "factorial", [ConstPropValue::Const(5)])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result, ConstPropValue::Top);
    }

    // --- A custom total ABSTRACT frame enum --------------------------------
    //
    // The abstract analogue of `TracingFrame`: it reuses the standard abstract
    // frames verbatim (via `AbstractFrameBuild` + the `*_into` methods) and adds
    // observation. The engine is not forked — only `AbstractInterpreter`'s `F`
    // type parameter changes. This proves abstract *traversal* is frame-
    // parametric, distinct from the analysis-policy `P` budget customized above.

    thread_local! {
        static ATRACE: RefCell<AbstractTrace> = const {
            RefCell::new(AbstractTrace {
                function_steps: 0,
                cfg_steps: 0,
                block_steps: 0,
                if_steps: 0,
                calls: 0,
            })
        };
    }

    #[derive(Clone, Copy, Default)]
    struct AbstractTrace {
        function_steps: usize,
        cfg_steps: usize,
        block_steps: usize,
        if_steps: usize,
        calls: usize,
    }

    enum TracingAbstractFrame<V, E, K> {
        Function(AbstractFunctionFrame<V, E, K>),
        Cfg(AbstractCfgFrame<V, E, K>),
        Block(AbstractBlockFrame<V, E, K>),
        Call(AbstractCallFrame<V, E, K>),
        ScfIf(AbstractScfIfFrame<V, E, K>),
        ScfFor(AbstractScfForFrame<V, E, K>),
    }

    impl<V, E, K> AbstractFrameBuild<V, E, K> for TracingAbstractFrame<V, E, K> {
        fn from_function(frame: AbstractFunctionFrame<V, E, K>) -> Self {
            TracingAbstractFrame::Function(frame)
        }
        fn from_cfg(frame: AbstractCfgFrame<V, E, K>) -> Self {
            TracingAbstractFrame::Cfg(frame)
        }
        fn from_block(frame: AbstractBlockFrame<V, E, K>) -> Self {
            TracingAbstractFrame::Block(frame)
        }
        fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self {
            TracingAbstractFrame::Call(frame)
        }
    }

    impl<V, E, K> BuildAbstractScfIf<V, E, K> for TracingAbstractFrame<V, E, K> {
        fn scf_if(frame: AbstractScfIfFrame<V, E, K>) -> Self {
            TracingAbstractFrame::ScfIf(frame)
        }
    }

    impl<V, E, K> BuildAbstractScfFor<V, E, K> for TracingAbstractFrame<V, E, K> {
        fn scf_for(frame: AbstractScfForFrame<V, E, K>) -> Self {
            TracingAbstractFrame::ScfFor(frame)
        }
    }

    impl<I, V, E, K> Frame<I> for TracingAbstractFrame<V, E, K>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>
            + ForwardInterp<Frame = TracingAbstractFrame<V, E, K>>,
        V: Clone + PartialEq + ForLoopValue,
        E: From<InterpreterError>,
        K: Clone + Eq + Hash,
    {
        type Completion = AbstractCompletion<V>;

        fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingAbstractFrame::Function(frame) => {
                    ATRACE.with(|t| t.borrow_mut().function_steps += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingAbstractFrame::Cfg(frame) => {
                    ATRACE.with(|t| t.borrow_mut().cfg_steps += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingAbstractFrame::Block(frame) => {
                    ATRACE.with(|t| t.borrow_mut().block_steps += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingAbstractFrame::Call(frame) => {
                    ATRACE.with(|t| t.borrow_mut().calls += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingAbstractFrame::ScfIf(frame) => {
                    ATRACE.with(|t| t.borrow_mut().if_steps += 1);
                    frame.step_into::<I, Self>(interp)
                }
                TracingAbstractFrame::ScfFor(frame) => frame.step_into::<I, Self>(interp),
            }
        }

        fn resume_done(
            self,
            _interp: &mut I,
        ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingAbstractFrame::Function(frame) => Ok(frame.resume_done_into::<Self>()),
                TracingAbstractFrame::Cfg(frame) => Ok(frame.resume_done_into::<Self>()),
                TracingAbstractFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
                TracingAbstractFrame::Call(frame) => frame.resume_done_into::<Self>(),
                TracingAbstractFrame::ScfIf(frame) => frame.resume_done_into::<Self>(),
                TracingAbstractFrame::ScfFor(frame) => frame.resume_done_into::<Self>(),
            }
        }

        fn resume(
            self,
            completion: Self::Completion,
            interp: &mut I,
        ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
            match self {
                TracingAbstractFrame::Function(frame) => frame.resume_into::<Self>(completion),
                TracingAbstractFrame::Cfg(frame) => {
                    frame.resume_into::<I, Self>(completion, interp)
                }
                TracingAbstractFrame::Block(frame) => {
                    frame.resume_into::<I, Self>(completion, interp)
                }
                TracingAbstractFrame::Call(frame) => frame.resume_into::<Self>(completion),
                TracingAbstractFrame::ScfIf(frame) => {
                    frame.resume_into::<I, Self>(completion, interp)
                }
                TracingAbstractFrame::ScfFor(frame) => {
                    frame.resume_into::<I, Self>(completion, interp)
                }
            }
        }
    }

    type CpKey = <ConstPropContext as CallContext<ConstPropValue>>::Key;

    type TracingAnalysis<'ir> = AbstractInterpreter<
        'ir,
        Stage,
        ConstPropValue,
        ToyError,
        CrossStageLinker,
        ConstPropContext,
        TracingAbstractFrame<ConstPropValue, ToyError, CpKey>,
    >;

    #[test]
    fn custom_abstract_frame_analyzes_program_and_observes_traversal() {
        ATRACE.with(|t| *t.borrow_mut() = AbstractTrace::default());
        let pipeline = build_pipeline(include_str!("../../programs/factorial.kirin"));

        // AbstractInterpreter parameterized by the *custom* abstract frame enum.
        let mut analysis: TracingAnalysis<'_> =
            AbstractInterpreter::new(&pipeline).with_linker(CrossStageLinker);
        let result = expect_single::<ConstPropValue, ToyError>(
            analysis
                .analyze_by_name("source", "factorial", [ConstPropValue::Const(5)])
                .unwrap(),
        )
        .unwrap();

        // (1)+(2): the custom abstract frame ran the real interprocedural fixpoint
        // correctly by reusing the standard abstract frames — precise recursive
        // constant propagation, no engine fork.
        assert_eq!(result, ConstPropValue::Const(120));

        // (3): abstract traversal is observable through the custom frame — a real
        // frame type `F`, not merely a custom analysis policy `P`. Counts are not
        // pinned (the interprocedural fixpoint re-enqueues summaries).
        let trace = ATRACE.with(|t| *t.borrow());
        assert!(trace.function_steps > 0, "function frames must be stepped");
        assert!(trace.cfg_steps > 0, "CFG frames must be stepped");
        assert!(trace.calls > 0, "call frames must be stepped");
    }
}
