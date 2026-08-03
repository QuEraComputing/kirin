//! Acceptance tests for generic interpreter bodies (issue #667).
//!
//! Two independent axes organize concrete traversal:
//!
//! - **Body representation** — the closed `Body` vocabulary: `CFG`, `Block`,
//!   `DiGraph`, `UnGraph`. Each framework-walkable representation has one
//!   walker (`CFGFrame`/`BlockFrame`/`DiGraphFrame`); `UnGraph` traversal is
//!   a compiler-supplied policy (`FrameBuild::from_ungraph_entry`).
//! - **Entry context** — callable (entered through `CallFrame`, which owns
//!   the callee activation) vs. nested (entered through a dialect frame
//!   pushed with `SparseForwardEffect::Push`, borrowing the current
//!   activation).
//!
//! The tests cover the composition matrix: callable CFG/Block/DiGraph bodies
//! (`CallFrame` → walker), nested DiGraph and scf Blocks (dialect frame →
//! walker), returns bubbling through dialect frames to the nearest
//! `CallFrame`, and the callable-UnGraph policy hook (with and without a
//! policy).
//!
//! The later sections run the *forward dataflow* engine
//! (`SparseForwardInterpreter`, instantiated at the constant-propagation
//! lattice) over the same body vocabulary: `CFG`, `Block` and `DiGraph`
//! callable bodies analyze — the last as an `Owner::Graph` walked by
//! `AbstractDiGraphFrame` — while `UnGraph` bodies and *nested* graph bodies
//! are asserted to be refused. Those sections also pin the graph owner's
//! interprocedural behaviour: calls *inside* a graph body are summarized rather
//! than descended into (so self-recursion converges), entry arguments join and
//! re-run the owner when several call sites share one key, and a directed cycle
//! is rejected identically by both engines.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::Hash;

use kirin::prelude::*;
use kirin_arith::{
    Arith, ArithConversionError, ArithType, ArithValue, interpreter::DivisionByZero,
};
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_constprop::{ConstPropContext, ConstPropValue};
use kirin_function::Lexical;
use kirin_interpreter::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractDiGraphFrame,
    AbstractFrameBuild, AbstractFrameDriver, BlockFrame, Body, BodyFrameEntry, CFGFrame,
    CallBodyFramePolicy, CallContext, CallFrame, Completion, ConcreteInterpreter,
    ContextInsensitive, DefaultBodyFrames, DiGraphFrame, Env, EnvIndex, Frame, FrameBuild,
    FrameDriver, FrameEffect, FunctionEntry, Interpretable, InterpreterError, SameStageLinker,
    SparseForwardEffect, SparseForwardInterp, SparseForwardInterpreter, StandardFrame,
    UnGraphEntry, expect_single,
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
// `ConstPropValue: From<ArithValue>` is infallible, so the abstract engine's
// `TryFrom<ArithValue>` conversion error is uninhabited.
impl From<std::convert::Infallible> for TestError {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
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
    expect_single(run_product(pipeline, function, args)?)
}

/// `run`, keeping the whole returned product — for callables that return more
/// than one value.
fn run_product(
    pipeline: &Pipeline<L>,
    function: &str,
    args: &[i64],
) -> Result<Product<i64>, TestError> {
    let mut interp: Engine<'_> = ConcreteInterpreter::new(pipeline).with_linker(SameStageLinker);
    interp.call_by_name("test", function, args.iter().copied())
}

// ===========================================================================
// 1. Callable DiGraph: caller → CallFrame → DiGraphFrame.
// ===========================================================================

/// A CFG-bodied `main` calls a DiGraph-bodied callable. The `call.named`
/// statement produces a `Call` effect; the resulting `CallFrame` resolves
/// the callee, allocates its activation, and — because the callable body is
/// `Body::DiGraph` — enters a `DiGraphFrame`. The graph walks its arith
/// nodes in dependency order and completes `Finished` with its declared
/// yields, which the `CallFrame` accepts as the call's returned values and
/// writes into `main`'s result slots.
const DIGRAPH_CALLABLE_PROGRAM: &str = r#"
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
"#;

#[test]
fn cfg_main_calls_digraph_function() {
    let pipeline = parse(DIGRAPH_CALLABLE_PROGRAM);
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 5);
}

// ===========================================================================
// 2. Nested/pushed DiGraph: dialect operation → DiGraphFrame.
// ===========================================================================

/// A statement inside a CFG block owns a DiGraph body and enters it with
/// `SparseForwardEffect::Push` — the same way `scf.if` enters its Block
/// arms. Unlike test 1 there is **no** `CallFrame` in the chain: no callee
/// resolution happens, no function activation is allocated or freed — the
/// pushed `DiGraphFrame` runs in the *pusher's* activation, and its
/// `Finished` yields land in the pushing statement's `Push` result slots
/// rather than in call-return slots.
const NESTED_DIGRAPH_PROGRAM: &str = r#"
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
"#;

#[test]
fn cfg_statement_pushes_digraph_body() {
    let pipeline = parse(NESTED_DIGRAPH_PROGRAM);
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
const BLOCK_CALLABLE_PROGRAM: &str = r#"
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
"#;

#[test]
fn linear_block_callable() {
    let pipeline = parse(BLOCK_CALLABLE_PROGRAM);
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
const DIGRAPH_TOPO_PROGRAM: &str = r#"
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
"#;

#[test]
fn digraph_runs_in_topological_order() {
    let pipeline = parse(DIGRAPH_TOPO_PROGRAM);
    // (3 + 3) * (3 + 1) = 24 — requires running both `add`s (and the
    // constant) before the `mul` despite the textual order.
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 24);
}

// ===========================================================================
// An scf-composed language for tests 5 and 6: structured operations enter
// nested Blocks through dialect frames (ScfIfFrame/ScfForFrame → BlockFrame).
// ===========================================================================

/// Inline language wrapping functions (CFG bodies), scf, and arithmetic.
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
#[derive(FrameBuild)]
enum ScfTestFrame<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
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

impl<I, F, V, E> Frame<I, F> for ScfTestFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = F>,
    F: FrameBuild<V, E, BodyFrames = DefaultBodyFrames> + BuildScfIf<V, E> + BuildScfFor<V, E>,
    V: Clone + kirin_scf::ForLoopValue,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            ScfTestFrame::Block(frame) => frame.step_into(interp),
            ScfTestFrame::CFG(frame) => frame.step_into(interp),
            ScfTestFrame::Call(frame) => frame.step_into(interp),
            ScfTestFrame::DiGraph(frame) => frame.step_into(interp),
            ScfTestFrame::ScfIf(frame) => frame.step_into(interp),
            ScfTestFrame::ScfFor(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            ScfTestFrame::Block(frame) => frame.resume_done_into(interp),
            ScfTestFrame::CFG(frame) => frame.resume_done_into(interp),
            ScfTestFrame::Call(frame) => frame.resume_done_into(interp),
            ScfTestFrame::DiGraph(frame) => frame.resume_done_into(interp),
            ScfTestFrame::ScfIf(frame) => frame.resume_done_into(interp),
            ScfTestFrame::ScfFor(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            ScfTestFrame::Block(frame) => frame.resume_into(completion, interp),
            ScfTestFrame::CFG(frame) => frame.resume_into(completion, interp),
            ScfTestFrame::Call(frame) => frame.resume_into(completion, interp),
            ScfTestFrame::DiGraph(frame) => frame.resume_into(completion, interp),
            ScfTestFrame::ScfIf(frame) => frame.resume_into(completion, interp),
            ScfTestFrame::ScfFor(frame) => frame.resume_into(completion, interp),
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
const SCF_ABS_PROGRAM: &str = r#"
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
"#;

#[test]
fn scf_if_arm_yields_to_dialect_frame() {
    let pipeline = parse_scf(SCF_ABS_PROGRAM);
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
/// boundary), the function's `CFGFrame` relays it too, and the nearest
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
    CFG(CFGFrame<i64, TestError>),
    Call(CallFrame<i64>),
    DiGraph(DiGraphFrame<i64, TestError>),
    Chain(UnGraphChainFrame),
}

impl FrameBuild<i64, TestError> for UnPolicyFrame {
    type BodyFrames = DefaultBodyFrames;

    fn from_block(frame: BlockFrame<i64, TestError>) -> Self {
        UnPolicyFrame::Block(frame)
    }
    fn from_cfg(frame: CFGFrame<i64, TestError>) -> Self {
        UnPolicyFrame::CFG(frame)
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
}

impl<'ir> Frame<UnEngine<'ir>, UnPolicyFrame> for UnGraphChainFrame {
    type Completion = Completion<i64>;

    fn step_into(
        mut self,
        interp: &mut UnEngine<'ir>,
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

    fn resume_done_into(
        self,
        _interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<UnPolicyFrame, Completion<i64>>, TestError> {
        Err(TestError::Core(InterpreterError::Custom(
            "the chain policy pushes no children",
        )))
    }

    fn resume_into(
        self,
        _completion: Completion<i64>,
        _interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<UnPolicyFrame, Completion<i64>>, TestError> {
        Err(TestError::Core(InterpreterError::Custom(
            "the chain policy pushes no children",
        )))
    }
}

type UnEngine<'ir> = ConcreteInterpreter<'ir, L, i64, TestError, SameStageLinker, UnPolicyFrame>;

// Pinned to `F = Self` rather than generic: the `Chain` variant holds the
// compiler-supplied `UnGraphChainFrame`, which has no `*FrameBuild` hook (it is
// constructed through `FrameBuild::from_ungraph_entry`), so there is no way to
// re-wrap it into an arbitrary outer `F`.
impl<'ir> Frame<UnEngine<'ir>, Self> for UnPolicyFrame {
    type Completion = Completion<i64>;

    fn step_into(
        self,
        interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<Self, Completion<i64>>, TestError> {
        match self {
            UnPolicyFrame::Block(frame) => frame.step_into(interp),
            UnPolicyFrame::CFG(frame) => frame.step_into(interp),
            UnPolicyFrame::Call(frame) => frame.step_into(interp),
            UnPolicyFrame::DiGraph(frame) => frame.step_into(interp),
            UnPolicyFrame::Chain(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(
        self,
        interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<Self, Completion<i64>>, TestError> {
        match self {
            UnPolicyFrame::Block(frame) => frame.resume_done_into(interp),
            UnPolicyFrame::CFG(frame) => frame.resume_done_into(interp),
            UnPolicyFrame::Call(frame) => frame.resume_done_into(interp),
            UnPolicyFrame::DiGraph(frame) => frame.resume_done_into(interp),
            UnPolicyFrame::Chain(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: Completion<i64>,
        interp: &mut UnEngine<'ir>,
    ) -> Result<FrameEffect<Self, Completion<i64>>, TestError> {
        match self {
            UnPolicyFrame::Block(frame) => frame.resume_into(completion, interp),
            UnPolicyFrame::CFG(frame) => frame.resume_into(completion, interp),
            UnPolicyFrame::Call(frame) => frame.resume_into(completion, interp),
            UnPolicyFrame::DiGraph(frame) => frame.resume_into(completion, interp),
            UnPolicyFrame::Chain(frame) => frame.resume_into(completion, interp),
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

// ===========================================================================
// 9. The same bodies under forward dataflow (abstract interpretation).
// ===========================================================================

// The tests above pin *concrete* traversal. These pin what the forward
// dataflow engine does with the same closed `Body` vocabulary, at the
// constant-propagation lattice:
//
// - `CFG`, `Block` and `DiGraph` bodies analyze. The engine translates the
//   callable body into the executable owner the worklist holds — `Body::CFG` →
//   its entry block, `Body::Block` → itself, `Body::DiGraph` → an
//   `Owner::Graph` walked by `AbstractDiGraphFrame` as one dependency-ordered
//   pass (exact for a DAG, so no intra-graph widening).
// - `UnGraph` bodies are **refused**: an undirected graph has no derivable
//   traversal order, and unlike the concrete engine there is no seam through
//   which a compiler could supply one.
// - A *nested* graph body (`graph_eval`) is also still refused, for an
//   unrelated reason: that dialect rule builds the **concrete** `DiGraphFrame`
//   directly instead of selecting one per engine the way scf does, so no
//   abstract walker can be substituted. Both refusals are asserted rather than
//   left implicit.

/// Summary key of the constant-propagation context policy.
type CpKey = <ConstPropContext as CallContext<ConstPropValue>>::Key;

/// Total abstract frame for the graph language: the standard abstract
/// traversal (blocks, calls, graph bodies), plus a variant recording the
/// absence of a walker.
///
/// The language's `Interpretable` rule bounds `I::Frame: FrameBuild<..>`
/// because its `graph_eval` variant pushes a **concrete** `DiGraphFrame`, so an
/// abstract frame type for this language must satisfy that bound even though
/// the abstract engine itself only ever calls `AbstractFrameBuild`. The
/// concrete walkers cannot be embedded here — their completion type is
/// `Completion<V>`, not `AbstractCompletion<V>` — so the `FrameBuild` hooks
/// build `NoWalker`, which reports the gap if it is ever stepped instead of
/// silently running concrete traversal over lattice values. Giving `graph_eval`
/// a per-engine dispatch trait (as `kirin-scf` does for `scf.if`/`scf.for`)
/// would remove the need for both the bound and this variant.
#[derive(AbstractFrameBuild)]
enum GraphAbstractFrame<V, E, K> {
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
    DiGraph(AbstractDiGraphFrame<V, E, K>),
    /// No abstract walker exists for this body kind; carries the reason.
    NoWalker(&'static str),
}

impl<V, E, K> FrameBuild<V, E> for GraphAbstractFrame<V, E, K> {
    type BodyFrames = DefaultBodyFrames;

    fn from_block(_: BlockFrame<V, E>) -> Self {
        GraphAbstractFrame::NoWalker("no abstract walker for a concrete Block frame")
    }
    fn from_cfg(_: CFGFrame<V, E>) -> Self {
        GraphAbstractFrame::NoWalker("no abstract walker for a concrete CFG frame")
    }
    fn from_call(_: CallFrame<V>) -> Self {
        GraphAbstractFrame::NoWalker("no abstract walker for a concrete call boundary")
    }
    fn from_digraph(_: DiGraphFrame<V, E>) -> Self {
        GraphAbstractFrame::NoWalker("no abstract digraph walker")
    }
}

impl<I, F, V, E, K> Frame<I, F> for GraphAbstractFrame<V, E, K>
where
    I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K> + SparseForwardInterp<Frame = F>,
    F: AbstractFrameBuild<V, E, K>,
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    type Completion = AbstractCompletion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        match self {
            GraphAbstractFrame::Block(frame) => frame.step_into(interp),
            GraphAbstractFrame::Call(frame) => frame.step_into(interp),
            GraphAbstractFrame::DiGraph(frame) => frame.step_into(interp),
            GraphAbstractFrame::NoWalker(reason) => Err(E::from(InterpreterError::Custom(reason))),
        }
    }

    fn resume_done_into(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        match self {
            GraphAbstractFrame::Block(frame) => frame.resume_done_into(interp),
            GraphAbstractFrame::Call(frame) => frame.resume_done_into(interp),
            GraphAbstractFrame::DiGraph(frame) => frame.resume_done_into(interp),
            GraphAbstractFrame::NoWalker(reason) => Err(E::from(InterpreterError::Custom(reason))),
        }
    }

    fn resume_into(
        self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        match self {
            GraphAbstractFrame::Block(frame) => frame.resume_into(completion, interp),
            GraphAbstractFrame::Call(frame) => frame.resume_into(completion, interp),
            GraphAbstractFrame::DiGraph(frame) => frame.resume_into(completion, interp),
            GraphAbstractFrame::NoWalker(reason) => Err(E::from(InterpreterError::Custom(reason))),
        }
    }
}

type AbstractEngine<'ir> = SparseForwardInterpreter<
    'ir,
    L,
    ConstPropValue,
    TestError,
    SameStageLinker,
    ConstPropContext,
    GraphAbstractFrame<ConstPropValue, TestError, CpKey>,
>;

/// Run constant propagation from `function` and return its inferred return
/// value at the fixpoint.
fn analyze(
    pipeline: &Pipeline<L>,
    function: &str,
    args: &[ConstPropValue],
) -> Result<ConstPropValue, TestError> {
    let mut analysis: AbstractEngine<'_> =
        SparseForwardInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(analysis.analyze_by_name("test", function, args.iter().cloned())?)
}

/// Summary key of the *context-insensitive* policy: one key per function, so
/// every call site shares one owner and their arguments join.
type CiKey = <ContextInsensitive as CallContext<ConstPropValue>>::Key;

type InsensitiveEngine<'ir> = SparseForwardInterpreter<
    'ir,
    L,
    ConstPropValue,
    TestError,
    SameStageLinker,
    ContextInsensitive,
    GraphAbstractFrame<ConstPropValue, TestError, CiKey>,
>;

/// The same analysis under [`ContextInsensitive`] keying: distinct call sites
/// collapse onto one owner, so entry arguments must join and the owner must be
/// re-analyzed when they rise.
fn analyze_insensitive(
    pipeline: &Pipeline<L>,
    function: &str,
    args: &[ConstPropValue],
) -> Result<ConstPropValue, TestError> {
    let mut analysis: InsensitiveEngine<'_> =
        SparseForwardInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(analysis.analyze_by_name("test", function, args.iter().cloned())?)
}

/// A CFG body whose branch condition is an *unknown* argument, so neither
/// successor can be decided: the abstract block frame explores both and joins
/// their returns. Identical arms fold to a constant; differing arms join to
/// `Top`.
const CFG_BRANCH_PROGRAM: &str = r#"
stage @test fn @same(i64) -> i64;
stage @test fn @diff(i64) -> i64;
stage @test fn @caller(i64) -> i64;

specialize @test fn @same(i64) -> i64 {
  ^entry(%c: i64) {
    cond_br %c then=^t() else=^f();
  }
  ^t() {
    %a = constant 7 -> i64;
    ret %a;
  }
  ^f() {
    %b = constant 7 -> i64;
    ret %b;
  }
}

specialize @test fn @diff(i64) -> i64 {
  ^entry(%c: i64) {
    cond_br %c then=^t() else=^f();
  }
  ^t() {
    %a = constant 7 -> i64;
    ret %a;
  }
  ^f() {
    %b = constant 9 -> i64;
    ret %b;
  }
}

specialize @test fn @caller(i64) -> i64 {
  ^entry(%c: i64) {
    %r = call.named @same(%c) -> i64;
    %one = constant 1 -> i64;
    %s = add %r, %one -> i64;
    ret %s;
  }
}
"#;

/// `Body::CFG` under forward dataflow: the entry block is seeded, the
/// undecided `cond_br` explores both successors, and the returns are joined.
#[test]
fn abstract_cfg_body_joins_branch_arms() {
    let pipeline = parse(CFG_BRANCH_PROGRAM);
    // Both arms return the same constant, so the join stays precise even
    // though the condition is unknown.
    assert_eq!(
        analyze(&pipeline, "same", &[ConstPropValue::Top]).unwrap(),
        ConstPropValue::Const(7)
    );
    // Differing arms join to Top — evidence both were actually explored
    // rather than one being picked.
    assert_eq!(
        analyze(&pipeline, "diff", &[ConstPropValue::Top]).unwrap(),
        ConstPropValue::Top
    );
}

/// The interprocedural path: a CFG-bodied caller summarizes a CFG-bodied
/// callee and folds the returned summary into its own arithmetic.
#[test]
fn abstract_cfg_body_summarizes_call() {
    let pipeline = parse(CFG_BRANCH_PROGRAM);
    assert_eq!(
        analyze(&pipeline, "caller", &[ConstPropValue::Top]).unwrap(),
        ConstPropValue::Const(8)
    );
}

/// `Body::Block` under forward dataflow: a single-block callable is seeded
/// directly as its own entry block (no CFG entry lookup), and is reached both
/// as an analysis root and as a summarized callee.
#[test]
fn abstract_block_body_callable() {
    let pipeline = parse(BLOCK_CALLABLE_PROGRAM);
    assert_eq!(
        analyze(
            &pipeline,
            "ladd",
            &[ConstPropValue::Const(40), ConstPropValue::Const(2)]
        )
        .unwrap(),
        ConstPropValue::Const(42)
    );
    // Same body, reached through a call from a CFG-bodied caller.
    assert_eq!(
        analyze(&pipeline, "main", &[]).unwrap(),
        ConstPropValue::Const(42)
    );
    // An unknown operand propagates through the block body to Top.
    assert_eq!(
        analyze(
            &pipeline,
            "ladd",
            &[ConstPropValue::Top, ConstPropValue::Const(2)]
        )
        .unwrap(),
        ConstPropValue::Top
    );
}

/// `Body::DiGraph` under forward dataflow: the callable graph body becomes an
/// `Owner::Graph` walked by `AbstractDiGraphFrame` — one dependency-ordered
/// pass binding the boundary ports, with the graph's declared yields becoming
/// the function's return summary. Reached both as an analysis root and as a
/// summarized callee.
#[test]
fn abstract_digraph_callable_analyzes() {
    let pipeline = parse(DIGRAPH_CALLABLE_PROGRAM);
    assert_eq!(
        analyze(
            &pipeline,
            "gadd",
            &[ConstPropValue::Const(2), ConstPropValue::Const(3)]
        )
        .unwrap(),
        ConstPropValue::Const(5)
    );
    // Same body, reached through a call from a CFG-bodied caller: the graph
    // owner's yields flow back as the callee's return summary.
    assert_eq!(
        analyze(&pipeline, "main", &[]).unwrap(),
        ConstPropValue::Const(5)
    );
    // An unknown port value propagates through the graph to Top.
    assert_eq!(
        analyze(
            &pipeline,
            "gadd",
            &[ConstPropValue::Top, ConstPropValue::Const(3)]
        )
        .unwrap(),
        ConstPropValue::Top
    );
}

/// The abstract walker uses the same dependency schedule as the concrete one:
/// this graph's nodes are declared consumer-before-producer, so a textual walk
/// would read unbound operands.
#[test]
fn abstract_digraph_follows_dependency_order() {
    let pipeline = parse(DIGRAPH_TOPO_PROGRAM);
    // (3 + 3) * (3 + 1) = 24, matching `digraph_runs_in_topological_order`.
    assert_eq!(
        analyze(&pipeline, "g", &[ConstPropValue::Const(3)]).unwrap(),
        ConstPropValue::Const(24)
    );
    assert_eq!(
        analyze(&pipeline, "main", &[]).unwrap(),
        ConstPropValue::Const(24)
    );
}

/// A callable `UnGraph` body is refused on the same path. Note the concrete
/// escape hatch does *not* apply here: `FrameBuild::from_ungraph_entry` is a
/// concrete-frame hook, so an UnGraph policy supplied for execution buys
/// nothing under abstract interpretation.
#[test]
fn abstract_ungraph_callable_reports_no_default_walker() {
    let pipeline = parse(UNGRAPH_PROGRAM);
    let error = analyze(&pipeline, "main", &[]).unwrap_err();
    assert!(
        matches!(
            error,
            TestError::Core(InterpreterError::NoDefaultWalker(Body::UnGraph(_)))
        ),
        "expected NoDefaultWalker(UnGraph), got {error:?}"
    );
}

/// A *nested* graph body is refused too, by a different route: the statement's
/// rule pushes a walker, and the abstract frame type has none to give. The
/// callable cases above never reach a frame at all, so this is the only test
/// covering the pushed path.
#[test]
fn abstract_nested_digraph_reports_no_walker() {
    let pipeline = parse(NESTED_DIGRAPH_PROGRAM);
    let error = analyze(&pipeline, "main", &[]).unwrap_err();
    assert!(
        matches!(
            error,
            TestError::Core(InterpreterError::Custom("no abstract digraph walker"))
        ),
        "expected the pushed-frame walker gap, got {error:?}"
    );
}

// ===========================================================================
// 10. Calls *inside* a graph body.
// ===========================================================================

/// A digraph node that is itself a call. The dependency edge `%a → %b` runs
/// through two call results, so the graph walker must sequence the calls, and
/// each call must be routed through the engine's call protocol rather than
/// evaluated inline.
const DIGRAPH_CALLS_PROGRAM: &str = r#"
stage @test fn @inc(i64) -> i64;
stage @test fn @gcall(i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @inc(i64) -> i64 {
  ^entry(%v: i64) {
    %one = constant 1 -> i64;
    %s = add %v, %one -> i64;
    ret %s;
  }
}

specialize @test fn @gcall(i64) -> i64 digraph ^g0(%x: i64) {
  %b = call.named @inc(%a) -> i64;
  %a = call.named @inc(%x) -> i64;
  yield %b;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %c = constant 5 -> i64;
    %r = call.named @gcall(%c) -> i64;
    ret %r;
  }
}
"#;

/// Concretely: the `DiGraphFrame` pushes a `CallFrame` per call node, in
/// dependency order (`%a` before `%b` despite the textual order).
#[test]
fn digraph_node_calls_run_in_dependency_order() {
    let pipeline = parse(DIGRAPH_CALLS_PROGRAM);
    assert_eq!(run(&pipeline, "gcall", &[5]).unwrap(), 7);
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 7);
}

/// Abstractly the same graph must route each call through
/// `summarize_call` — an `AbstractCallFrame`, *not* the concrete `CallFrame`,
/// which would descend into the callee and bypass the interprocedural
/// protocol. This is the one arm where `AbstractDiGraphFrame` differs
/// substantively from the concrete walker.
#[test]
fn abstract_digraph_node_calls_are_summarized() {
    let pipeline = parse(DIGRAPH_CALLS_PROGRAM);
    assert_eq!(
        analyze(&pipeline, "gcall", &[ConstPropValue::Const(5)]).unwrap(),
        ConstPropValue::Const(7)
    );
    assert_eq!(
        analyze(&pipeline, "main", &[]).unwrap(),
        ConstPropValue::Const(7)
    );
    // An unknown port flows through both summarized calls to Top.
    assert_eq!(
        analyze(&pipeline, "gcall", &[ConstPropValue::Top]).unwrap(),
        ConstPropValue::Top
    );
}

/// A graph body that calls *itself*. A digraph cannot branch, so this
/// recursion has no base case and does not terminate concretely — which is
/// exactly why it is analysis-only. It terminates here because the call is
/// summarized: the self-key's return summary starts at `bottom` and the owner
/// re-runs only while it rises. Descending into the callee instead would
/// recurse forever.
const DIGRAPH_RECURSION_PROGRAM: &str = r#"
stage @test fn @grec(i64) -> i64;

specialize @test fn @grec(i64) -> i64 digraph ^g0(%x: i64) {
  %r = call.named @grec(%x) -> i64;
  yield %r;
}
"#;

#[test]
fn abstract_digraph_self_recursion_converges() {
    let pipeline = parse(DIGRAPH_RECURSION_PROGRAM);
    // Never returns, so the sound fixpoint is `bottom` — and, crucially, the
    // analysis reaches it instead of diverging.
    assert_eq!(
        analyze(&pipeline, "grec", &[ConstPropValue::Const(1)]).unwrap(),
        ConstPropValue::Bottom
    );
}

// ===========================================================================
// 11. Graph-owner re-analysis: two call sites, one owner.
// ===========================================================================

/// One graph-bodied callee reached from two call sites with different
/// constants.
const DIGRAPH_TWO_CALLERS_PROGRAM: &str = r#"
stage @test fn @gdouble(i64) -> i64;
stage @test fn @twocalls() -> i64;

specialize @test fn @gdouble(i64) -> i64 digraph ^g0(%x: i64) {
  %s = add %x, %x -> i64;
  yield %s;
}

specialize @test fn @twocalls() -> i64 {
  ^entry() {
    %a = constant 1 -> i64;
    %b = constant 2 -> i64;
    %p = call.named @gdouble(%a) -> i64;
    %q = call.named @gdouble(%b) -> i64;
    %s = add %p, %q -> i64;
    ret %s;
  }
}
"#;

/// The convergence behaviour of a graph owner, pinned from both sides by
/// running the same program under both keying policies.
///
/// Under [`ContextInsensitive`] the two call sites share one owner, so its
/// entry product must **join** (`Const(1) ⊔ Const(2)` = `Top`) and the graph
/// must be **re-analyzed** with the wider entry — an owner seeded once and
/// never re-run would leave the second call site reading a stale `Const(4)`.
/// Under `ConstPropContext` the sites key separately and each stays exact.
/// Together these show a graph owner participates in entry widening exactly
/// like a block owner, which is where all of its convergence pressure comes
/// from (one dependency-ordered pass is exact, so nothing widens *inside* the
/// graph).
#[test]
fn abstract_digraph_owner_joins_two_call_sites() {
    let pipeline = parse(DIGRAPH_TWO_CALLERS_PROGRAM);
    // Shared owner: entry joins to Top, the graph re-runs, both results are Top.
    assert_eq!(
        analyze_insensitive(&pipeline, "twocalls", &[]).unwrap(),
        ConstPropValue::Top
    );
    // Distinct keys: 1+1 = 2 and 2+2 = 4, so 2 + 4 = 6.
    assert_eq!(
        analyze(&pipeline, "twocalls", &[]).unwrap(),
        ConstPropValue::Const(6)
    );
}

// ===========================================================================
// 12. Cyclic DiGraph: rejected when the walk plan is built.
// ===========================================================================

/// A digraph whose nodes depend on each other. The IR represents this happily —
/// it parses — because a `DiGraph` is not required to be acyclic.
const DIGRAPH_CYCLE_PROGRAM: &str = r#"
stage @test fn @gcycle(i64) -> i64;

specialize @test fn @gcycle(i64) -> i64 digraph ^g0(%x: i64) {
  %a = add %b, %x -> i64;
  %b = add %a, %x -> i64;
  yield %a;
}
"#;

/// Both engines reject a directed cycle, with the *same* error and from the
/// *same* place: `digraph_walk_plan` topologically sorts the nodes, and a
/// cyclic graph has no topological order. So the rejection is a property of the
/// walk plan (shared by the concrete and abstract walkers), not of the IR and
/// not of either engine.
///
/// Supporting cyclic graph bodies is therefore not an extension of the current
/// walkers: it needs a schedule that is not a toposort plus a fixpoint *inside*
/// the graph, which in turn needs a finer unit of re-analysis than
/// `Owner::Graph`'s single exact pass.
#[test]
fn cyclic_digraph_is_rejected_by_both_engines() {
    let pipeline = parse(DIGRAPH_CYCLE_PROGRAM);

    let concrete = run(&pipeline, "gcycle", &[1]).unwrap_err();
    assert!(
        matches!(
            concrete,
            TestError::Core(InterpreterError::GraphHasCycle(_))
        ),
        "expected GraphHasCycle concretely, got {concrete:?}"
    );

    let abstract_ = analyze(&pipeline, "gcycle", &[ConstPropValue::Const(1)]).unwrap_err();
    assert!(
        matches!(
            abstract_,
            TestError::Core(InterpreterError::GraphHasCycle(_))
        ),
        "expected GraphHasCycle abstractly, got {abstract_:?}"
    );
}

// ===========================================================================
// 13. Multi-yield graphs and boundary arity.
// ===========================================================================

/// A graph yielding **two** values into a two-result call site.
///
/// Note the declared signature is `-> i64`, one type, while the function
/// actually returns two values: `Signature` carries a single `ret` type, so it
/// does not constrain return *arity*. The product arity that matters at runtime
/// is the graph's `yield` list versus the call statement's result slots. (This
/// is the same shape as `example/toy-qc/programs/ghz.kirin`, which declares
/// `-> Qubit` and yields three.)
const DIGRAPH_MULTIYIELD_PROGRAM: &str = r#"
stage @test fn @gpair(i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @gpair(i64) -> i64 digraph ^g0(%x: i64) {
  %a = add %x, %x -> i64;
  %b = mul %x, %x -> i64;
  yield %a, %b;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %c = constant 3 -> i64;
    %p, %q = call.named @gpair(%c) -> i64, i64;
    %s = add %p, %q -> i64;
    ret %s;
  }
}
"#;

/// Yield order is result-slot order: `%p` gets the first yield, `%q` the
/// second. Asserting the product directly (rather than only their sum) pins
/// that mapping.
#[test]
fn digraph_yields_multiple_values() {
    let pipeline = parse(DIGRAPH_MULTIYIELD_PROGRAM);
    let values: Vec<i64> = run_product(&pipeline, "gpair", &[3])
        .unwrap()
        .iter()
        .copied()
        .collect();
    // 3 + 3 = 6 and 3 * 3 = 9, in yield order.
    assert_eq!(values, vec![6, 9]);
    // Both land in the caller's two result slots: 6 + 9.
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 15);
}

/// The abstract walker collects the same product, so a multi-result callee's
/// return summary carries both slots.
#[test]
fn abstract_digraph_yields_multiple_values() {
    let pipeline = parse(DIGRAPH_MULTIYIELD_PROGRAM);
    assert_eq!(
        analyze(&pipeline, "main", &[]).unwrap(),
        ConstPropValue::Const(15)
    );
}

/// A graph whose boundary ports outnumber the call's arguments. Nothing earlier
/// in the pipeline cross-checks the declared signature against the port list, so
/// the graph walkers arity-check when binding the ports — the same check, and
/// the same error, in both engines.
const DIGRAPH_PORT_ARITY_PROGRAM: &str = r#"
stage @test fn @g2(i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @g2(i64) -> i64 digraph ^g0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  yield %s;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %c = constant 3 -> i64;
    %r = call.named @g2(%c) -> i64;
    ret %r;
  }
}
"#;

#[test]
fn digraph_port_arity_mismatch_is_reported() {
    let pipeline = parse(DIGRAPH_PORT_ARITY_PROGRAM);

    let concrete = run(&pipeline, "main", &[]).unwrap_err();
    assert!(
        matches!(
            concrete,
            TestError::Core(InterpreterError::ProductArityMismatch {
                expected: 2,
                actual: 1
            })
        ),
        "expected a port arity mismatch concretely, got {concrete:?}"
    );

    let abstract_ = analyze(&pipeline, "main", &[]).unwrap_err();
    assert!(
        matches!(
            abstract_,
            TestError::Core(InterpreterError::ProductArityMismatch {
                expected: 2,
                actual: 1
            })
        ),
        "expected a port arity mismatch abstractly, got {abstract_:?}"
    );
}

// ===========================================================================
// 14. A custom callable-body walker policy.
// ===========================================================================

// `CallFrame` owns the call convention — resolve, allocate, enter, suspend,
// validate the completion, free the activation exactly once, bind results — and
// delegates only *which walker enters the callee body* to a
// `CallBodyFramePolicy`. These tests show a language replacing that choice for
// two body kinds without reimplementing any of the lifecycle, and confirm the
// choice does not leak into `scf.if`, which picks its own dialect frame.

thread_local! {
    /// Which body kinds the custom policy was asked for, in order.
    static POLICY_LOG: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

/// A custom policy: instrument `CFG` and `DiGraph` entry, delegate `Block` and
/// `UnGraph` to the framework default. Each arm still builds the *standard*
/// walker — the point is that the language chose it, not that it walks
/// differently.
struct LoggingBodyFrames;

impl CallBodyFramePolicy<i64, TestError, PolicyFrame> for LoggingBodyFrames {
    fn from_cfg(entry: BodyFrameEntry<CFG, i64>) -> Result<PolicyFrame, TestError> {
        POLICY_LOG.with(|log| log.borrow_mut().push("cfg"));
        Ok(PolicyFrame::CFG(CFGFrame::new(
            entry.stage,
            entry.index,
            entry.body,
            entry.args,
        )))
    }

    fn from_block(entry: BodyFrameEntry<Block, i64>) -> Result<PolicyFrame, TestError> {
        POLICY_LOG.with(|log| log.borrow_mut().push("block"));
        <DefaultBodyFrames as CallBodyFramePolicy<i64, TestError, PolicyFrame>>::from_block(entry)
    }

    fn from_digraph(entry: BodyFrameEntry<DiGraph, i64>) -> Result<PolicyFrame, TestError> {
        POLICY_LOG.with(|log| log.borrow_mut().push("digraph"));
        Ok(PolicyFrame::DiGraph(DiGraphFrame::new(
            entry.stage,
            entry.index,
            entry.body,
            entry.args,
        )))
    }

    fn from_ungraph(entry: BodyFrameEntry<UnGraph, i64>) -> Result<PolicyFrame, TestError> {
        POLICY_LOG.with(|log| log.borrow_mut().push("ungraph"));
        <DefaultBodyFrames as CallBodyFramePolicy<i64, TestError, PolicyFrame>>::from_ungraph(entry)
    }
}

/// A total frame type that selects `LoggingBodyFrames`. Note the `Call` variant
/// carries the policy, and `FrameBuild::BodyFrames` names it — the derive would
/// emit exactly this via `#[interpret(body_frames = LoggingBodyFrames)]`.
enum PolicyFrame {
    Block(BlockFrame<i64, TestError>),
    CFG(CFGFrame<i64, TestError>),
    Call(CallFrame<i64, LoggingBodyFrames>),
    DiGraph(DiGraphFrame<i64, TestError>),
    ScfIf(ScfIfFrame<i64, TestError>),
    ScfFor(ScfForFrame<i64, TestError>),
}

impl FrameBuild<i64, TestError> for PolicyFrame {
    type BodyFrames = LoggingBodyFrames;

    fn from_block(frame: BlockFrame<i64, TestError>) -> Self {
        PolicyFrame::Block(frame)
    }
    fn from_cfg(frame: CFGFrame<i64, TestError>) -> Self {
        PolicyFrame::CFG(frame)
    }
    fn from_call(frame: CallFrame<i64, LoggingBodyFrames>) -> Self {
        PolicyFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<i64, TestError>) -> Self {
        PolicyFrame::DiGraph(frame)
    }
}

impl BuildScfIf<i64, TestError> for PolicyFrame {
    fn scf_if(frame: ScfIfFrame<i64, TestError>) -> Self {
        PolicyFrame::ScfIf(frame)
    }
}

impl BuildScfFor<i64, TestError> for PolicyFrame {
    fn scf_for(frame: ScfForFrame<i64, TestError>) -> Self {
        PolicyFrame::ScfFor(frame)
    }
}

impl<I> Frame<I, PolicyFrame> for PolicyFrame
where
    I: FrameDriver<Value = i64, Error = TestError> + SparseForwardInterp<Frame = PolicyFrame>,
{
    type Completion = Completion<i64>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Completion<i64>>, TestError> {
        match self {
            PolicyFrame::Block(frame) => frame.step_into(interp),
            PolicyFrame::CFG(frame) => frame.step_into(interp),
            PolicyFrame::Call(frame) => frame.step_into(interp),
            PolicyFrame::DiGraph(frame) => frame.step_into(interp),
            PolicyFrame::ScfIf(frame) => frame.step_into(interp),
            PolicyFrame::ScfFor(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Completion<i64>>, TestError> {
        match self {
            PolicyFrame::Block(frame) => frame.resume_done_into(interp),
            PolicyFrame::CFG(frame) => frame.resume_done_into(interp),
            PolicyFrame::Call(frame) => frame.resume_done_into(interp),
            PolicyFrame::DiGraph(frame) => frame.resume_done_into(interp),
            PolicyFrame::ScfIf(frame) => frame.resume_done_into(interp),
            PolicyFrame::ScfFor(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: Completion<i64>,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Completion<i64>>, TestError> {
        match self {
            PolicyFrame::Block(frame) => frame.resume_into(completion, interp),
            PolicyFrame::CFG(frame) => frame.resume_into(completion, interp),
            PolicyFrame::Call(frame) => frame.resume_into(completion, interp),
            PolicyFrame::DiGraph(frame) => frame.resume_into(completion, interp),
            PolicyFrame::ScfIf(frame) => frame.resume_into(completion, interp),
            PolicyFrame::ScfFor(frame) => frame.resume_into(completion, interp),
        }
    }
}

type PolicyEngine<'ir> = ConcreteInterpreter<'ir, L, i64, TestError, SameStageLinker, PolicyFrame>;

fn run_with_policy(pipeline: &Pipeline<L>, function: &str, args: &[i64]) -> Result<i64, TestError> {
    POLICY_LOG.with(|log| log.borrow_mut().clear());
    let mut interp: PolicyEngine<'_> =
        ConcreteInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(interp.call_by_name("test", function, args.iter().copied())?)
}

fn policy_log() -> Vec<&'static str> {
    POLICY_LOG.with(|log| log.borrow().clone())
}

/// A root call and a nested call, both routed through the custom policy. The
/// returned values are unchanged — only the *selection* of the walker moved.
#[test]
fn custom_body_policy_enters_callable_bodies() {
    let pipeline = parse(DIGRAPH_CALLABLE_PROGRAM);

    // Root call into a CFG body, which then calls a DiGraph body.
    assert_eq!(run_with_policy(&pipeline, "main", &[]).unwrap(), 5);
    assert_eq!(policy_log(), vec!["cfg", "digraph"]);

    // Root call straight into the DiGraph body.
    assert_eq!(run_with_policy(&pipeline, "gadd", &[2, 3]).unwrap(), 5);
    assert_eq!(policy_log(), vec!["digraph"]);
}

/// The `Block` arm delegates to `DefaultBodyFrames`, so a policy can override
/// only the body kinds it cares about.
#[test]
fn custom_body_policy_can_delegate_to_the_default() {
    let pipeline = parse(BLOCK_CALLABLE_PROGRAM);
    assert_eq!(run_with_policy(&pipeline, "main", &[]).unwrap(), 42);
    assert_eq!(policy_log(), vec!["cfg", "block"]);
}

/// Isolation: `scf.if` builds its *own* dialect frame via `ScfIfDispatch` and
/// walks the chosen arm with a framework `BlockFrame`. It is a nested body, not
/// a callable one, so the call-body policy must never be consulted for it.
#[test]
fn scf_if_does_not_use_the_call_body_policy() {
    let pipeline = parse_scf(SCF_ABS_PROGRAM);
    let mut interp: ConcreteInterpreter<
        '_,
        ScfL,
        i64,
        TestError,
        SameStageLinker,
        ScfTestFrame<i64, TestError>,
    > = ConcreteInterpreter::new(&pipeline).with_linker(SameStageLinker);
    POLICY_LOG.with(|log| log.borrow_mut().clear());
    let result =
        expect_single::<i64, TestError>(interp.call_by_name("test", "abs", [-7]).unwrap()).unwrap();
    assert_eq!(result, 7);
    // `ScfTestFrame` uses the default policy, and in any case the scf arm never
    // reaches a call boundary — the log stays empty.
    assert!(policy_log().is_empty(), "got {:?}", policy_log());
}

// ===========================================================================
// 15. A *genuine* replacement walker, selected through the derive.
// ===========================================================================

// Section 14 proves the policy is consulted, but each arm still built a
// standard walker and the total frame type implemented `FrameBuild` by hand. This
// section closes both gaps:
//
// - `MyCfgWalker` is a **distinct frame type** with its own `Frame` impl and its
//   own enum variant. A callable `CFG` body enters through it, never through
//   `DerivedFrame::CFG`.
// - the policy is selected by `#[interpret(body_frames = MyBodyFrames)]`, so the
//   derive is **compiled and executed** here rather than only snapshotted.
//
// What this does *not* claim: `MyCfgWalker` delegates the actual block-to-block
// traversal to a `CFGFrame` it owns, rather than reimplementing CFG walking. It
// hands off after the entry step, which is why the assertions count *entries*.
// The substitutable thing Roger asked for is the frame type the language chooses
// at the callable-body boundary, and that is what is replaced.

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct CustomTrace {
    /// `MyBodyFrames::from_cfg` calls — one per callable CFG activation.
    policy_selections: usize,
    /// Steps taken *by* `MyCfgWalker`.
    my_steps: usize,
    /// Steps taken by the standard `DerivedFrame::CFG` variant. Must stay `0`:
    /// if the custom walker ever handed the walk back to the framework variant,
    /// this counts it.
    standard_cfg_steps: usize,
}

thread_local! {
    static CUSTOM: RefCell<CustomTrace> = const { RefCell::new(CustomTrace {
        policy_selections: 0,
        my_steps: 0,
        standard_cfg_steps: 0,
    }) };
}

/// A language's own callable-CFG walker: distinct type, distinct variant.
struct MyCfgWalker<V, E> {
    inner: CFGFrame<V, E>,
}

impl<V, E> MyCfgWalker<V, E> {
    /// The inner `CFGFrame` re-wraps *itself* through `FrameBuild::from_cfg`,
    /// which lands in `DerivedFrame::CFG`. Lift those back so this walker stays
    /// resident for the whole traversal rather than only its entry step.
    ///
    /// Only the frame that represents *self* is lifted: `Push.child` is a
    /// different frame (a call boundary or a pushed dialect frame) and must be
    /// left alone.
    fn stay_resident(
        effect: FrameEffect<DerivedFrame<V, E>, Completion<V>>,
    ) -> FrameEffect<DerivedFrame<V, E>, Completion<V>> {
        fn lift<V, E>(frame: DerivedFrame<V, E>) -> DerivedFrame<V, E> {
            match frame {
                DerivedFrame::CFG(inner) => DerivedFrame::MyCfg(MyCfgWalker { inner }),
                other => other,
            }
        }
        match effect {
            FrameEffect::Continue(frame) => FrameEffect::Continue(lift(frame)),
            FrameEffect::Push { parent, child } => FrameEffect::Push {
                parent: lift(parent),
                child,
            },
            // `Done`/`Complete` carry no frame.
            other => other,
        }
    }
}

impl<I, V, E> Frame<I, DerivedFrame<V, E>> for MyCfgWalker<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = DerivedFrame<V, E>>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<DerivedFrame<V, E>, Completion<V>>, E> {
        CUSTOM.with(|t| t.borrow_mut().my_steps += 1);
        self.inner.step_into(interp).map(Self::stay_resident)
    }

    fn resume_done_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<DerivedFrame<V, E>, Completion<V>>, E> {
        CUSTOM.with(|t| t.borrow_mut().my_steps += 1);
        self.inner.resume_done_into(interp).map(Self::stay_resident)
    }

    fn resume_into(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<DerivedFrame<V, E>, Completion<V>>, E> {
        CUSTOM.with(|t| t.borrow_mut().my_steps += 1);
        self.inner
            .resume_into(completion, interp)
            .map(Self::stay_resident)
    }
}

/// Replaces the `CFG` walker outright; delegates the other three body kinds to
/// the framework default.
struct MyBodyFrames;

impl<V, E> CallBodyFramePolicy<V, E, DerivedFrame<V, E>> for MyBodyFrames
where
    V: Clone,
    E: From<InterpreterError>,
{
    fn from_cfg(entry: BodyFrameEntry<CFG, V>) -> Result<DerivedFrame<V, E>, E> {
        // Not `F::from_cfg` — the language's own walker, in its own variant.
        CUSTOM.with(|t| t.borrow_mut().policy_selections += 1);
        Ok(DerivedFrame::MyCfg(MyCfgWalker {
            inner: CFGFrame::new(entry.stage, entry.index, entry.body, entry.args),
        }))
    }

    fn from_block(entry: BodyFrameEntry<Block, V>) -> Result<DerivedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, DerivedFrame<V, E>>>::from_block(entry)
    }

    fn from_digraph(entry: BodyFrameEntry<DiGraph, V>) -> Result<DerivedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, DerivedFrame<V, E>>>::from_digraph(entry)
    }

    fn from_ungraph(entry: BodyFrameEntry<UnGraph, V>) -> Result<DerivedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, DerivedFrame<V, E>>>::from_ungraph(entry)
    }
}

/// The policy is chosen by the attribute; the derive emits
/// `type BodyFrames = MyBodyFrames` and the four injection constructors.
#[derive(FrameBuild)]
#[interpret(body_frames = MyBodyFrames)]
enum DerivedFrame<V, E> {
    Block(BlockFrame<V, E>),
    /// Required by `FrameBuild`, and reached only when `MyCfgWalker` hands off
    /// mid-walk — never as the entry frame for a callable body.
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V, MyBodyFrames>),
    DiGraph(DiGraphFrame<V, E>),
    MyCfg(MyCfgWalker<V, E>),
}

impl<I, V, E> Frame<I, Self> for DerivedFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = DerivedFrame<V, E>>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Completion<V>>, E> {
        match self {
            DerivedFrame::Block(frame) => frame.step_into(interp),
            DerivedFrame::CFG(frame) => {
                CUSTOM.with(|t| t.borrow_mut().standard_cfg_steps += 1);
                frame.step_into(interp)
            }
            DerivedFrame::Call(frame) => frame.step_into(interp),
            DerivedFrame::DiGraph(frame) => frame.step_into(interp),
            DerivedFrame::MyCfg(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(self, interp: &mut I) -> Result<FrameEffect<Self, Completion<V>>, E> {
        match self {
            DerivedFrame::Block(frame) => frame.resume_done_into(interp),
            DerivedFrame::CFG(frame) => frame.resume_done_into(interp),
            DerivedFrame::Call(frame) => frame.resume_done_into(interp),
            DerivedFrame::DiGraph(frame) => frame.resume_done_into(interp),
            DerivedFrame::MyCfg(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Completion<V>>, E> {
        match self {
            DerivedFrame::Block(frame) => frame.resume_into(completion, interp),
            DerivedFrame::CFG(frame) => frame.resume_into(completion, interp),
            DerivedFrame::Call(frame) => frame.resume_into(completion, interp),
            DerivedFrame::DiGraph(frame) => frame.resume_into(completion, interp),
            DerivedFrame::MyCfg(frame) => frame.resume_into(completion, interp),
        }
    }
}

fn run_derived(pipeline: &Pipeline<L>, function: &str, args: &[i64]) -> Result<i64, TestError> {
    CUSTOM.with(|t| *t.borrow_mut() = CustomTrace::default());
    let mut interp: ConcreteInterpreter<
        '_,
        L,
        i64,
        TestError,
        SameStageLinker,
        DerivedFrame<i64, TestError>,
    > = ConcreteInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(interp.call_by_name("test", function, args.iter().copied())?)
}

/// The custom walker is selected by `#[interpret(body_frames = ..)]` and stays
/// resident for the **whole** CFG traversal, because it re-wraps every frame the
/// inner walker hands back (see [`MyCfgWalker::stay_resident`]).
///
/// Two assertions carry the claim: `standard_cfg_steps == 0` proves the
/// framework's `CFGFrame` variant is never stepped, and `my_steps` well above
/// `policy_selections` proves the walker kept going rather than handing off after
/// entry. Deleting the re-wrap flips this to
/// `my_steps: 1, standard_cfg_steps: 4` — i.e. entry-only — so the assertions
/// have teeth. (Observed here: `my_steps: 6, standard_cfg_steps: 0`.)
#[test]
fn derived_policy_substitutes_a_custom_cfg_walker() {
    let pipeline = parse(DIGRAPH_CALLABLE_PROGRAM);

    // `main` is CFG-bodied → the custom walker. It calls `gadd`, a DiGraph body,
    // which the policy delegates to the framework's `DiGraphFrame`.
    assert_eq!(run_derived(&pipeline, "main", &[]).unwrap(), 5);
    let trace = CUSTOM.with(|t| *t.borrow());
    assert_eq!(trace.policy_selections, 1, "{trace:?}");
    assert_eq!(
        trace.standard_cfg_steps, 0,
        "custom walker leaked: {trace:?}"
    );
    assert!(
        trace.my_steps > trace.policy_selections,
        "walker handed off after entry: {trace:?}"
    );

    // A root call straight into the DiGraph body: delegated, so no custom CFG
    // walker is ever built.
    assert_eq!(run_derived(&pipeline, "gadd", &[2, 3]).unwrap(), 5);
    assert_eq!(CUSTOM.with(|t| *t.borrow()), CustomTrace::default());
}

/// Two nested CFG-bodied callables: the policy is consulted once per activation,
/// and neither activation ever falls back to the standard variant.
#[test]
fn derived_policy_walker_stays_resident_across_activations() {
    let pipeline = parse(CFG_BRANCH_PROGRAM);
    // `caller` (CFG) calls `same` (CFG) → two callable CFG activations.
    assert_eq!(run_derived(&pipeline, "caller", &[0]).unwrap(), 8);
    let trace = CUSTOM.with(|t| *t.borrow());
    assert_eq!(trace.policy_selections, 2, "{trace:?}");
    assert_eq!(
        trace.standard_cfg_steps, 0,
        "custom walker leaked: {trace:?}"
    );
    // Both bodies are multi-statement, so residency means many more steps than
    // the two entries.
    assert!(trace.my_steps > 4, "{trace:?}");
}
