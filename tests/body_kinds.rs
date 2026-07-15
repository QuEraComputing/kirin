//! Acceptance tests for generic interpreter bodies (issue #667).
//!
//! Two independent axes organize concrete traversal:
//!
//! - **Body representation** — the closed `Body` vocabulary: `Cfg`, `Block`,
//!   `DiGraph`, `UnGraph`. Each framework-walkable representation has one
//!   walker (`CfgFrame`/`BlockFrame`/`DiGraphFrame`); `UnGraph` traversal is
//!   a compiler-supplied policy (`FrameBuild::from_ungraph_entry`).
//! - **Entry context** — callable (entered through `CallFrame`, which owns
//!   the callee activation) vs. nested (entered through a dialect frame
//!   pushed with `SparseForwardEffect::Push`, borrowing the current
//!   activation).
//!
//! The tests cover the composition matrix: callable Cfg/Block/DiGraph bodies
//! (`CallFrame` → walker), nested DiGraph and scf Blocks (dialect frame →
//! walker), returns bubbling through dialect frames to the nearest
//! `CallFrame`, and the callable-UnGraph policy hook (with and without a
//! policy).

use std::collections::VecDeque;

use kirin::prelude::*;
use kirin_arith::{
    Arith, ArithConversionError, ArithType, ArithValue, interpreter::DivisionByZero,
};
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::Lexical;
use kirin_interpreter::{
    BlockFrame, Body, CallFrame, CfgFrame, Completion, ConcreteInterpreter, DiGraphFrame, Env,
    EnvIndex, Frame, FrameBuild, FrameDriver, FrameEffect, FunctionEntry, Interpretable,
    InterpreterError, SameStageLinker, SparseForwardEffect, StandardFrame, UnGraphEntry,
    expect_single,
};
use kirin_scf::{BuildScfFor, BuildScfIf, ScfForFrame, ScfIfFrame, StructuredControlFlow};
use kirin_test_languages::GraphFunctionLanguage;

/// Total error for the test engines: the framework error plus the value
/// conversion/trap errors the languages' rules can raise.
#[derive(Debug)]
enum TestError {
    Core(InterpreterError),
    ArithConversion(ArithConversionError),
    DivisionByZero,
}

impl From<InterpreterError> for TestError {
    fn from(error: InterpreterError) -> Self {
        Self::Core(error)
    }
}
impl From<ArithConversionError> for TestError {
    fn from(error: ArithConversionError) -> Self {
        Self::ArithConversion(error)
    }
}
impl From<DivisionByZero> for TestError {
    fn from(_: DivisionByZero) -> Self {
        Self::DivisionByZero
    }
}

type L = StageInfo<GraphFunctionLanguage>;
type Engine<'ir> =
    ConcreteInterpreter<'ir, L, i64, TestError, SameStageLinker, StandardFrame<i64, TestError>>;

fn parse(program: &str) -> Pipeline<L> {
    let mut pipeline: Pipeline<L> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
    pipeline
}

fn run(pipeline: &Pipeline<L>, function: &str, args: &[i64]) -> Result<i64, TestError> {
    let mut interp: Engine<'_> = ConcreteInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(interp.call_by_name("test", function, args.iter().copied())?)
}

// ===========================================================================
// 1. Callable DiGraph: caller → CallFrame → DiGraphFrame.
// ===========================================================================

/// A Cfg-bodied `main` calls a DiGraph-bodied callable. The `call.named`
/// statement produces a `Call` effect; the resulting `CallFrame` resolves
/// the callee, allocates its activation, and — because the callable body is
/// `Body::DiGraph` — enters a `DiGraphFrame`. The graph walks its arith
/// nodes in dependency order and completes `Finished` with its declared
/// yields, which the `CallFrame` accepts as the call's returned values and
/// writes into `main`'s result slots.
#[test]
fn cfg_main_calls_digraph_function() {
    let pipeline = parse(
        r#"
stage @test fn @gadd(i64, i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @gadd(i64, i64) -> i64 digraph ^g0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  yield %s;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 2 -> i64;
    %b = constant 3 -> i64;
    %r = call.named @gadd(%a, %b) -> i64;
    ret %r;
  }
}
"#,
    );
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 5);
}

// ===========================================================================
// 2. Nested/pushed DiGraph: dialect operation → DiGraphFrame.
// ===========================================================================

/// A statement inside a Cfg block owns a DiGraph body and enters it with
/// `SparseForwardEffect::Push` — the same way `scf.if` enters its Block
/// arms. Unlike test 1 there is **no** `CallFrame` in the chain: no callee
/// resolution happens, no function activation is allocated or freed — the
/// pushed `DiGraphFrame` runs in the *pusher's* activation, and its
/// `Finished` yields land in the pushing statement's `Push` result slots
/// rather than in call-return slots.
#[test]
fn cfg_statement_pushes_digraph_body() {
    let pipeline = parse(
        r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 20 -> i64;
    %b = constant 22 -> i64;
    %r = graph_eval %a, %b digraph ^g0(%x: i64, %y: i64) {
      %s = add %x, %y -> i64;
      yield %s;
    } -> i64;
    ret %r;
  }
}
"#,
    );
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 42);
}

// ===========================================================================
// 3. Callable Block: caller → CallFrame → BlockFrame.
// ===========================================================================

/// A linear (single-Block) callable: the flat-instruction-list function
/// shape. `Body::Block` maps to the same
/// `BlockFrame` that walks nested structured blocks — there is no separate
/// "linear function frame"; the `CallFrame` parent is what makes this walk a
/// function body. The block exits with `Return`, which the `CallFrame`
/// validates and consumes.
#[test]
fn linear_block_callable() {
    let pipeline = parse(
        r#"
stage @test fn @ladd(i64, i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @ladd(i64, i64) -> i64 ^body(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  ret %s;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 40 -> i64;
    %b = constant 2 -> i64;
    %r = call.named @ladd(%a, %b) -> i64;
    ret %r;
  }
}
"#,
    );
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 42);
}

// ===========================================================================
// 4. DiGraph dependency order.
// ===========================================================================

/// Graph nodes run in dependency order, not declaration order. The graph
/// branches visibly: one input port feeds two independent producers declared
/// *after* their consumer, whose results merge into the yielded node — so a
/// textual/linear walk would read unbound operands.
///
/// ```text
///   %x ──┬─▶ %c = add %x, %x ──┐
///        │                     ├─▶ %d = mul %c, %e ──▶ yield
///        └─▶ %e = add %x, %one ┘
/// ```
#[test]
fn digraph_runs_in_topological_order() {
    let pipeline = parse(
        r#"
stage @test fn @g(i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @g(i64) -> i64 digraph ^g0(%x: i64) {
  %d = mul %c, %e -> i64;
  %c = add %x, %x -> i64;
  %e = add %x, %one -> i64;
  %one = constant 1 -> i64;
  yield %d;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 3 -> i64;
    %r = call.named @g(%a) -> i64;
    ret %r;
  }
}
"#,
    );
    // (3 + 3) * (3 + 1) = 24 — requires running both `add`s (and the
    // constant) before the `mul` despite the textual order.
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 24);
}

// ===========================================================================
// An scf-composed language for tests 5 and 6: structured operations enter
// nested Blocks through dialect frames (ScfIfFrame/ScfForFrame → BlockFrame).
// ===========================================================================

/// Inline language wrapping functions (Cfg bodies), scf, and arithmetic.
/// Specific to this integration suite; shared test dialects live in
/// `kirin-test-languages`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Dialect, FunctionEntry, HasParser, PrettyPrint, Interpretable,
)]
#[kirin(builders, type = ArithType)]
enum ScfLanguage {
    #[wraps]
    #[callable]
    Lexical(Lexical<ArithType>),
    #[wraps]
    Structured(StructuredControlFlow<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
}

/// Total frame enum for the scf tests: the standard representation walkers
/// and call boundary plus the dialect-owned SCF frames (composition, not an
/// engine fork).
enum ScfTestFrame<V, E> {
    Block(BlockFrame<V, E>),
    Cfg(CfgFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
}

impl<V, E> FrameBuild<V, E> for ScfTestFrame<V, E> {
    fn from_block(frame: BlockFrame<V, E>) -> Self {
        ScfTestFrame::Block(frame)
    }
    fn from_cfg(frame: CfgFrame<V, E>) -> Self {
        ScfTestFrame::Cfg(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        ScfTestFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self {
        ScfTestFrame::DiGraph(frame)
    }
}

impl<V, E> BuildScfIf<V, E> for ScfTestFrame<V, E> {
    fn scf_if(frame: ScfIfFrame<V, E>) -> Self {
        ScfTestFrame::ScfIf(frame)
    }
}

impl<V, E> BuildScfFor<V, E> for ScfTestFrame<V, E> {
    fn scf_for(frame: ScfForFrame<V, E>) -> Self {
        ScfTestFrame::ScfFor(frame)
    }
}

impl<I, V, E> Frame<I> for ScfTestFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E>
        + kirin_interpreter::SparseForwardInterp<Frame = ScfTestFrame<V, E>>,
    V: Clone + kirin_scf::ForLoopValue,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ScfTestFrame::Block(frame) => frame.step_into::<I, Self>(interp),
            ScfTestFrame::Cfg(frame) => frame.step_into::<I, Self>(interp),
            ScfTestFrame::Call(frame) => frame.step_into::<I, Self>(interp),
            ScfTestFrame::DiGraph(frame) => frame.step_into::<I, Self>(interp),
            ScfTestFrame::ScfIf(frame) => frame.step_into::<I, Self>(interp),
            ScfTestFrame::ScfFor(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ScfTestFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
            ScfTestFrame::Cfg(frame) => Ok(frame.resume_done_into::<Self>()),
            ScfTestFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(I::Error::from),
            ScfTestFrame::DiGraph(frame) => Ok(frame.resume_done_into::<Self>()),
            ScfTestFrame::ScfIf(frame) => frame.resume_done_into::<Self>(),
            ScfTestFrame::ScfFor(frame) => frame.resume_done_into::<Self>(),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ScfTestFrame::Block(frame) => frame.resume_into::<I, Self>(completion, interp),
            ScfTestFrame::Cfg(frame) => frame.resume_into::<I, Self>(completion, interp),
            ScfTestFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
            ScfTestFrame::DiGraph(frame) => frame.resume_into::<I, Self>(completion, interp),
            ScfTestFrame::ScfIf(frame) => frame.resume_into::<Self>(completion),
            ScfTestFrame::ScfFor(frame) => frame.resume_into::<I, Self>(completion, interp),
        }
    }
}

type ScfL = StageInfo<ScfLanguage>;
type ScfEngine<'ir> =
    ConcreteInterpreter<'ir, ScfL, i64, TestError, SameStageLinker, ScfTestFrame<i64, TestError>>;

fn parse_scf(program: &str) -> Pipeline<ScfL> {
    let mut pipeline: Pipeline<ScfL> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
    pipeline
}

fn run_scf(pipeline: &Pipeline<ScfL>, function: &str, args: &[i64]) -> Result<i64, TestError> {
    let mut interp: ScfEngine<'_> = ConcreteInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(interp.call_by_name("test", function, args.iter().copied())?)
}

// ===========================================================================
// 5. Nested structured Block: ScfIfFrame/ScfForFrame → BlockFrame.
// ===========================================================================

/// `scf.if` picks the decided arm and pushes the framework `BlockFrame` for
/// it; the arm's `yield` surfaces as `Completion::Yielded`, which the
/// `ScfIfFrame` consumes and hands to the pusher as the operation's results.
#[test]
fn scf_if_arm_yields_to_dialect_frame() {
    let pipeline = parse_scf(
        r#"
stage @test fn @abs(i64) -> i64;

specialize @test fn @abs(i64) -> i64 {
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
"#,
    );
    assert_eq!(run_scf(&pipeline, "abs", &[-7]).unwrap(), 7);
    assert_eq!(run_scf(&pipeline, "abs", &[4]).unwrap(), 4);
}

/// `scf.for` re-pushes the framework `BlockFrame` per iteration; each
/// `Completion::Yielded` carries the loop-carried values into the next turn,
/// and loop exit completes `Finished` to the pusher.
#[test]
fn scf_for_loop_carries_yielded_values() {
    let pipeline = parse_scf(
        r#"
stage @test fn @sum_below(i64) -> i64;

specialize @test fn @sum_below(i64) -> i64 {
  ^entry(%n: i64) {
    %zero = constant 0 -> i64;
    %one = constant 1 -> i64;
    %sum = for %zero in %zero..%n step %one iter_args(%zero) do ^body(%i: i64, %acc: i64) {
      %next = add %acc, %i -> i64;
      yield %next;
    } -> i64;
    ret %sum;
  }
}
"#,
    );
    // 0 + 1 + 2 + 3 + 4 = 10.
    assert_eq!(run_scf(&pipeline, "sum_below", &[5]).unwrap(), 10);
    // Zero iterations: the initial carried value flows through.
    assert_eq!(run_scf(&pipeline, "sum_below", &[0]).unwrap(), 0);
}

// ===========================================================================
// 6. Return through nested structured control.
// ===========================================================================

/// A function `Return` inside an `scf.if` arm: the arm's `BlockFrame`
/// completes `Returned`, the `ScfIfFrame` relays it (it is not a call
/// boundary), the function's `CfgFrame` relays it too, and the nearest
/// `CallFrame` consumes it — freeing the callee activation exactly once and
/// writing the caller's result slots. Execution must not continue after the
/// return: the statements below the `if` never run on the early-return path.
#[test]
fn return_bubbles_through_scf_frames_to_call_frame() {
    let pipeline = parse_scf(
        r#"
stage @test fn @clamp0(i64) -> i64;
stage @test fn @twice(i64) -> i64;

specialize @test fn @clamp0(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    %kept = if %is_neg then ^then() {
      ret %zero;
    } else ^else() {
      yield %x;
    } -> i64;
    %one = constant 1 -> i64;
    %r = add %kept, %one -> i64;
    ret %r;
  }
}

specialize @test fn @twice(i64) -> i64 {
  ^entry(%x: i64) {
    %a = call.named @clamp0(%x) -> i64;
    %b = call.named @clamp0(%x) -> i64;
    %s = add %a, %b -> i64;
    ret %s;
  }
}
"#,
    );
    // Early return: 0, not 0 + 1 — the add after the `if` did not run.
    assert_eq!(run_scf(&pipeline, "clamp0", &[-5]).unwrap(), 0);
    // Normal path: the arm yields, execution continues after the `if`.
    assert_eq!(run_scf(&pipeline, "clamp0", &[5]).unwrap(), 6);
    // Two nested calls taking the early-return path in one run: each callee
    // activation is freed exactly once and the caller's activation survives,
    // otherwise the second call (or the final add) would read freed state.
    assert_eq!(run_scf(&pipeline, "twice", &[-5]).unwrap(), 0);
    assert_eq!(run_scf(&pipeline, "twice", &[5]).unwrap(), 12);
}

// ===========================================================================
// 7. Custom callable-UnGraph policy: caller → CallFrame → policy frame.
// ===========================================================================

// The framework refuses to invent an execution order for an undirected
// graph, so the *compiler* supplies one by overriding
// `FrameBuild::from_ungraph_entry` on its total frame type. This policy
// interprets an ungraph as a sequential dataflow chain:
//
// - **scheduling**: node statements run in the graph's canonical node
//   enumeration order (an explicit policy choice — the generic `CallFrame`
//   never orders anything);
// - **outputs**: the call returns the result values of the *last* scheduled
//   node (an ungraph has no framework output convention such as a digraph's
//   declared yields, so the policy defines one).

/// Total frame enum for the UnGraph-policy engine: the standard frames plus
/// the policy's own walker. Only `from_ungraph_entry` differs from the
/// default composition — the generic traversal logic is untouched.
enum UnPolicyFrame {
    Block(BlockFrame<i64, TestError>),
    Cfg(CfgFrame<i64, TestError>),
    Call(CallFrame<i64>),
    DiGraph(DiGraphFrame<i64, TestError>),
    Chain(UnGraphChainFrame),
}

impl FrameBuild<i64, TestError> for UnPolicyFrame {
    fn from_block(frame: BlockFrame<i64, TestError>) -> Self {
        UnPolicyFrame::Block(frame)
    }
    fn from_cfg(frame: CfgFrame<i64, TestError>) -> Self {
        UnPolicyFrame::Cfg(frame)
    }
    fn from_call(frame: CallFrame<i64>) -> Self {
        UnPolicyFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<i64, TestError>) -> Self {
        UnPolicyFrame::DiGraph(frame)
    }
    fn from_ungraph_entry(entry: UnGraphEntry<i64>) -> Result<Self, TestError> {
        Ok(UnPolicyFrame::Chain(UnGraphChainFrame::new(entry)))
    }
}

/// The compiler-owned callable-UnGraph walker (the policy itself).
struct UnGraphChainFrame {
    stage: CompileStage,
    /// The callee activation — owned and freed by the awaiting `CallFrame`,
    /// merely used here.
    index: EnvIndex,
    graph: UnGraph,
    /// Entry arguments awaiting the boundary-port binding on the first step.
    pending: Option<Product<i64>>,
    /// The policy's schedule (canonical node enumeration order).
    schedule: VecDeque<Statement>,
    /// The policy's output convention: the last scheduled node's results.
    outputs: Vec<SSAValue>,
}

impl UnGraphChainFrame {
    fn new(entry: UnGraphEntry<i64>) -> Self {
        Self {
            stage: entry.stage,
            index: entry.index,
            graph: entry.graph,
            pending: Some(entry.args),
            schedule: VecDeque::new(),
            outputs: Vec::new(),
        }
    }

    fn step(
        mut self,
        interp: &mut UnEngine<'_>,
    ) -> Result<FrameEffect<UnPolicyFrame, Completion<i64>>, TestError> {
        // First step: bind the boundary ports and fix the policy's schedule
        // and output convention from the graph's structure.
        if let Some(args) = self.pending.take() {
            let info = interp
                .pipeline()
                .stage(self.stage)
                .ok_or(InterpreterError::MissingStage(self.stage))?;
            let graph_info = self
                .graph
                .get_info(info)
                .ok_or(InterpreterError::Custom("ungraph has no info"))?;
            if graph_info.ports().len() != args.len() {
                return Err(TestError::Core(InterpreterError::ProductArityMismatch {
                    expected: graph_info.ports().len(),
                    actual: args.len(),
                }));
            }
            let ports: Vec<_> = graph_info.ports().to_vec();
            let nodes: Vec<Statement> = graph_info.graph().node_weights().copied().collect();
            let last = *nodes
                .last()
                .ok_or(InterpreterError::Custom("empty ungraph body"))?;
            self.outputs = last
                .definition(info)
                .results()
                .map(|result| SSAValue::from(*result))
                .collect();
            self.schedule = nodes.into();
            for (port, value) in ports.into_iter().zip(args) {
                interp.env_write(self.index, SSAValue::from(port), value)?;
            }
            return Ok(FrameEffect::Continue(UnPolicyFrame::Chain(self)));
        }

        match self.schedule.pop_front() {
            Some(statement) => match interp.run_statement(self.stage, statement, self.index)? {
                SparseForwardEffect::Next => Ok(FrameEffect::Continue(UnPolicyFrame::Chain(self))),
                _ => Err(TestError::Core(InterpreterError::Custom(
                    "the chain policy supports only ordinary dataflow nodes",
                ))),
            },
            // Natural completion: the policy's outputs become the call's
            // returned values (the awaiting CallFrame accepts `Finished`).
            None => {
                let values: Product<i64> = self
                    .outputs
                    .iter()
                    .map(|&value| interp.env_read(self.index, value))
                    .collect::<Result<_, _>>()?;
                Ok(FrameEffect::Complete(Completion::Finished(values)))
            }
        }
    }
}

type UnEngine<'ir> = ConcreteInterpreter<'ir, L, i64, TestError, SameStageLinker, UnPolicyFrame>;

impl<'ir> Frame<UnEngine<'ir>> for UnPolicyFrame {
    type Completion = Completion<i64>;

    fn step(
        self,
        interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<Self, Self::Completion>, TestError> {
        match self {
            UnPolicyFrame::Block(frame) => frame.step_into::<UnEngine<'ir>, Self>(interp),
            UnPolicyFrame::Cfg(frame) => frame.step_into::<UnEngine<'ir>, Self>(interp),
            UnPolicyFrame::Call(frame) => frame.step_into::<UnEngine<'ir>, Self>(interp),
            UnPolicyFrame::DiGraph(frame) => frame.step_into::<UnEngine<'ir>, Self>(interp),
            UnPolicyFrame::Chain(frame) => frame.step(interp),
        }
    }

    fn resume_done(
        self,
        _interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<Self, Self::Completion>, TestError> {
        match self {
            UnPolicyFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
            UnPolicyFrame::Cfg(frame) => Ok(frame.resume_done_into::<Self>()),
            UnPolicyFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(TestError::from),
            UnPolicyFrame::DiGraph(frame) => Ok(frame.resume_done_into::<Self>()),
            UnPolicyFrame::Chain(_) => Err(TestError::Core(InterpreterError::Custom(
                "the chain policy pushes no children",
            ))),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<Self, Self::Completion>, TestError> {
        match self {
            UnPolicyFrame::Block(frame) => {
                frame.resume_into::<UnEngine<'ir>, Self>(completion, interp)
            }
            UnPolicyFrame::Cfg(frame) => {
                frame.resume_into::<UnEngine<'ir>, Self>(completion, interp)
            }
            UnPolicyFrame::Call(frame) => {
                frame.resume_into::<UnEngine<'ir>, Self>(completion, interp)
            }
            UnPolicyFrame::DiGraph(frame) => {
                frame.resume_into::<UnEngine<'ir>, Self>(completion, interp)
            }
            UnPolicyFrame::Chain(_) => Err(TestError::Core(InterpreterError::Custom(
                "the chain policy pushes no children",
            ))),
        }
    }
}

const UNGRAPH_PROGRAM: &str = r#"
stage @test fn @usq(i64, i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @usq(i64, i64) -> i64 ungraph ^u0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  %t = mul %s, %s -> i64;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 2 -> i64;
    %b = constant 3 -> i64;
    %r = call.named @usq(%a, %b) -> i64;
    ret %r;
  }
}
"#;

/// A language *with* an UnGraph policy: the call goes caller → `CallFrame` →
/// the compiler's `UnGraphChainFrame`. The `CallFrame` still owns the callee
/// activation and return bookkeeping; only the walker construction was
/// delegated.
#[test]
fn custom_ungraph_policy_is_callable() {
    let pipeline = parse(UNGRAPH_PROGRAM);
    let mut interp: UnEngine<'_> = ConcreteInterpreter::new(&pipeline);
    let result: i64 =
        expect_single::<i64, TestError>(interp.call_by_name("test", "main", []).unwrap()).unwrap();
    // (2 + 3)^2 = 25, via the policy's chain schedule and last-node outputs.
    assert_eq!(result, 25);
}

// ===========================================================================
// 8. UnGraph without a policy: a clear no-default-walker error.
// ===========================================================================

/// The standard frames supply no UnGraph traversal, so calling an
/// UnGraph-bodied function reports `NoDefaultWalker` instead of inventing a
/// node order.
#[test]
fn ungraph_without_policy_reports_no_default_walker() {
    let pipeline = parse(UNGRAPH_PROGRAM);
    let error = run(&pipeline, "main", &[]).unwrap_err();
    assert!(
        matches!(
            error,
            TestError::Core(InterpreterError::NoDefaultWalker(Body::UnGraph(_)))
        ),
        "expected NoDefaultWalker(UnGraph), got {error:?}"
    );
}
