//! Compile-time regression tests for the **engine-capability split**.
//!
//! Each engine here is *deliberately incomplete*: it implements only the
//! capability traits one kind of frame consumes, and omits the rest. The value
//! of this file is that **it compiles** — every `assert_frame` /
//! `assert_dataflow_engine` call below is a static proof that the named frame
//! does not secretly require a capability the engine never provides.
//!
//! Before the split there was one monolithic capability trait carrying every
//! operation, so *none* of these four engines could exist: running a block
//! walker meant also supplying `alloc_env`/`free_env`/`resolve_call`/
//! `enter_function`/`cfg_entry`/`digraph_walk_plan`, and an abstract dataflow
//! engine had to expose a concrete call convention it never performs.
//!
//! | mock engine | pins |
//! |---|---|
//! | `BlockOnlyEngine` | `BlockFrame` needs only `Env + StatementDispatch + BlockQueries` |
//! | `CallOnlyEngine` | `CallFrame` needs only `CallServices` — not even a statement-effect algebra |
//! | `AbstractOnlyEngine` | `ForwardDataflowFrameEngine` requires neither `CallServices` nor `CFGQueries` |
//! | `QueriesOnlyEngine` | the `*Queries` traits are honestly read-only: satisfiable with no `Env` at all |
//!
//! Each is load-bearing. Widening a member frame's bound (adding `CallServices`
//! to `BlockFrame`'s `Frame` impl), re-attaching the call lifecycle to the
//! abstract umbrella, or re-adding `Env` as a `*Queries` supertrait each stops
//! this file compiling and names what regressed.
//!
//! The mock engines panic if actually *run*: nothing here executes IR. That is
//! the point — these are type-level assertions, and the behavioral coverage
//! lives in `tests/body_kinds.rs` and the engine crates.

// Everything here exists to be *type-checked*, not read: the child variants
// prove the narrow conversion bounds are satisfiable and the storage exists
// only to satisfy `Env`, so "never read" is the expected state of this file.
#![allow(dead_code)]

use std::collections::HashMap;

use kirin_interpreter::{
    AbstractBlockFrame, AbstractCallFrame, AbstractDiGraphFrame, BlockFrame, BlockQueries,
    CFGFrame, CFGQueries, CallEffect, CallFrame, CallRequest, CallServices, CallableBody, Callee,
    DefaultCallBodyTraversal, DiGraphFrame, DiGraphQueries, Env, EnvIndex,
    ForwardDataflowFrameEngine, ForwardEval, ForwardFrameEngine, Frame, FunctionTarget, Interp,
    InterpreterError, SparseForwardEffect, StatementDispatch,
};
use kirin_ir::{Block, CompileStage, Product, SSAValue, Statement};

/// The compile-time assertions this file is made of.
///
/// None is ever called; instantiating them is what type-checks the bounds.
fn assert_frame<I, R, T>()
where
    I: kirin_interpreter::FrameEngine,
    T: Frame<I, R>,
{
}

fn assert_dataflow_engine<I: ForwardDataflowFrameEngine>() {}

/// The `*Queries` traits must be satisfiable **without** [`Env`] — that is what
/// makes their names truthful.
fn assert_read_only_queries<I: BlockQueries + CFGQueries + DiGraphQueries>() {}

/// Minimal child representation for the concrete member proofs.
enum CapabilityChild {
    Block(BlockFrame<i64, InterpreterError>),
    CFG(CFGFrame<i64, InterpreterError>),
    Call(CallRequest<i64>),
    DiGraph(DiGraphFrame<i64, InterpreterError>),
}

impl From<BlockFrame<i64, InterpreterError>> for CapabilityChild {
    fn from(frame: BlockFrame<i64, InterpreterError>) -> Self {
        Self::Block(frame)
    }
}

impl From<CFGFrame<i64, InterpreterError>> for CapabilityChild {
    fn from(frame: CFGFrame<i64, InterpreterError>) -> Self {
        Self::CFG(frame)
    }
}

impl From<CallRequest<i64>> for CapabilityChild {
    fn from(request: CallRequest<i64>) -> Self {
        Self::Call(request)
    }
}

impl From<DiGraphFrame<i64, InterpreterError>> for CapabilityChild {
    fn from(frame: DiGraphFrame<i64, InterpreterError>) -> Self {
        Self::DiGraph(frame)
    }
}

// ===========================================================================
// Shared mock storage
// ===========================================================================

/// Minimal SSA storage so the mocks can satisfy [`Env`] without pulling in the
/// real engines.
#[derive(Default)]
struct MockStore(HashMap<(usize, SSAValue), i64>);

// ===========================================================================
// 1. BlockOnlyEngine — walks blocks, and nothing else
// ===========================================================================

/// Implements: [`Interp`], [`Env`], [`StatementDispatch`], [`BlockQueries`].
///
/// **Deliberately omits**: [`CallServices`] (no `alloc_env`/`free_env`/
/// `resolve_call`/`enter_function`), [`CFGQueries`] (no
/// `cfg_entry`), and [`DiGraphQueries`] (no `digraph_walk_plan`).
///
/// So this engine cannot enter a function, cannot find a CFG's entry block, and
/// cannot schedule a graph — yet it can still run the block walker.
#[derive(Default)]
struct BlockOnlyEngine {
    store: MockStore,
}

impl Interp for BlockOnlyEngine {
    type Value = i64;
    type Error = InterpreterError;
    type Effect = SparseForwardEffect<i64, CapabilityChild>;
    type Semantics = ForwardEval;

    fn stage(&self) -> CompileStage {
        unimplemented!("type-level mock")
    }
    fn statement(&self) -> Statement {
        unimplemented!("type-level mock")
    }
    fn index(&self) -> EnvIndex {
        unimplemented!("type-level mock")
    }
}

impl Env for BlockOnlyEngine {
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<i64, InterpreterError> {
        self.store
            .0
            .get(&(index.raw(), value))
            .copied()
            .ok_or(InterpreterError::UnboundValue { index, value })
    }

    fn env_write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: i64,
    ) -> Result<(), InterpreterError> {
        self.store.0.insert((index.raw(), value), data);
        Ok(())
    }
}

impl StatementDispatch for BlockOnlyEngine {
    fn run_statement(
        &mut self,
        _stage: CompileStage,
        _statement: Statement,
        _index: EnvIndex,
    ) -> Result<Self::Effect, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

impl BlockQueries for BlockOnlyEngine {
    fn block_params(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Vec<SSAValue>, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn first_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn next_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
        _after: Statement,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

#[test]
fn block_frame_runs_on_an_engine_with_only_block_queries_and_dispatch() {
    assert_frame::<BlockOnlyEngine, CapabilityChild, BlockFrame<i64, InterpreterError>>();
}

// ===========================================================================
// 2. CallOnlyEngine — performs the call lifecycle, and nothing else
// ===========================================================================

/// Implements: [`Interp`], [`Env`], [`CallServices`].
///
/// **Deliberately omits**: [`StatementDispatch`] (cannot dispatch a
/// statement), [`BlockQueries`], [`CFGQueries`], and
/// [`DiGraphQueries`] (cannot query any body shape).
///
/// Its `Effect` is `()`, not a [`SparseForwardEffect`] — proof that
/// [`CallFrame`] needs neither a statement-effect algebra nor
/// [`SparseForwardInterp`](kirin_interpreter::SparseForwardInterp). The call
/// boundary only allocates, resolves, enters, suspends, frees, and binds
/// results.
#[derive(Default)]
struct CallOnlyEngine {
    store: MockStore,
}

impl Interp for CallOnlyEngine {
    type Value = i64;
    type Error = InterpreterError;
    type Effect = ();
    type Semantics = ForwardEval;

    fn stage(&self) -> CompileStage {
        unimplemented!("type-level mock")
    }
    fn statement(&self) -> Statement {
        unimplemented!("type-level mock")
    }
    fn index(&self) -> EnvIndex {
        unimplemented!("type-level mock")
    }
}

impl Env for CallOnlyEngine {
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<i64, InterpreterError> {
        self.store
            .0
            .get(&(index.raw(), value))
            .copied()
            .ok_or(InterpreterError::UnboundValue { index, value })
    }

    fn env_write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: i64,
    ) -> Result<(), InterpreterError> {
        self.store.0.insert((index.raw(), value), data);
        Ok(())
    }
}

impl CallServices for CallOnlyEngine {
    fn alloc_env(&mut self) -> EnvIndex {
        unimplemented!("type-level mock")
    }
    fn free_env(&mut self, _index: EnvIndex) -> Result<(), InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn resolve_call(
        &self,
        _stage: CompileStage,
        _callee: &Callee,
    ) -> Result<FunctionTarget, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn enter_function(
        &mut self,
        _stage: CompileStage,
        _body: Statement,
        _args: Product<i64>,
        _index: EnvIndex,
    ) -> Result<CallableBody<i64>, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

#[test]
fn call_frame_runs_on_an_engine_with_only_call_services() {
    assert_frame::<CallOnlyEngine, CapabilityChild, CallFrame<i64, DefaultCallBodyTraversal>>();
}

// ===========================================================================
// 3. AbstractOnlyEngine — abstract dataflow with no concrete call lifecycle
// ===========================================================================

/// Implements: [`Interp`], [`Env`], [`StatementDispatch`],
/// [`BlockQueries`], [`DiGraphQueries`], and
/// [`ForwardDataflowFrameEngine`].
///
/// **Deliberately omits**: [`CallServices`] and
/// [`CFGQueries`].
///
/// This is the assertion that carries item #3's main claim. An abstract engine
/// *summarizes* a call ([`ForwardDataflowFrameEngine::summarize_call`]) instead
/// of descending into it, and reaches a callable body's entry block through
/// owner seeding rather than `cfg_entry` — so it should not have to expose
/// activation allocation, activation cleanup, `enter_function`, `resolve_call`,
/// or `cfg_entry` merely to be an abstract dataflow engine. Before the split it
/// did.
#[derive(Default)]
struct AbstractOnlyEngine {
    store: MockStore,
}

impl Interp for AbstractOnlyEngine {
    type Value = i64;
    type Error = InterpreterError;
    type Effect = SparseForwardEffect<i64, MockAbstractFrame>;
    type Semantics = ForwardEval;

    fn stage(&self) -> CompileStage {
        unimplemented!("type-level mock")
    }
    fn statement(&self) -> Statement {
        unimplemented!("type-level mock")
    }
    fn index(&self) -> EnvIndex {
        unimplemented!("type-level mock")
    }
}

impl Env for AbstractOnlyEngine {
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<i64, InterpreterError> {
        self.store
            .0
            .get(&(index.raw(), value))
            .copied()
            .ok_or(InterpreterError::UnboundValue { index, value })
    }

    fn env_write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: i64,
    ) -> Result<(), InterpreterError> {
        self.store.0.insert((index.raw(), value), data);
        Ok(())
    }
}

impl StatementDispatch for AbstractOnlyEngine {
    fn run_statement(
        &mut self,
        _stage: CompileStage,
        _statement: Statement,
        _index: EnvIndex,
    ) -> Result<Self::Effect, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

impl BlockQueries for AbstractOnlyEngine {
    fn block_params(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Vec<SSAValue>, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn first_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn next_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
        _after: Statement,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

/// Taken as-is: the `NoDefaultWalker` default is the whole point of
/// [`DiGraphQueries`] being a separate capability an engine opts into.
impl DiGraphQueries for AbstractOnlyEngine {}

impl ForwardDataflowFrameEngine for AbstractOnlyEngine {
    type SummaryKey = ();

    fn analysis_merge(
        &self,
        _current: &Product<i64>,
        _incoming: &Product<i64>,
        _visits: usize,
    ) -> Result<Product<i64>, InterpreterError> {
        unimplemented!("type-level mock")
    }

    fn contribute_return(&mut self, _values: Product<i64>) -> Result<(), InterpreterError> {
        unimplemented!("type-level mock")
    }

    fn current_function_key(&self) -> Option<()> {
        unimplemented!("type-level mock")
    }

    fn summarize_call(
        &mut self,
        _stage: CompileStage,
        _call: CallEffect<i64>,
        _index: EnvIndex,
    ) -> Result<(), InterpreterError> {
        unimplemented!("type-level mock")
    }

    fn max_iterations(&self) -> usize {
        unimplemented!("type-level mock")
    }
}

/// Minimal abstract stack-item composition used to prove the member bounds.
enum MockAbstractFrame {
    Block(AbstractBlockFrame<i64, InterpreterError, ()>),
    Call(AbstractCallFrame<i64, InterpreterError, ()>),
    DiGraph(AbstractDiGraphFrame<i64, InterpreterError, ()>),
}

impl From<AbstractBlockFrame<i64, InterpreterError, ()>> for MockAbstractFrame {
    fn from(frame: AbstractBlockFrame<i64, InterpreterError, ()>) -> Self {
        Self::Block(frame)
    }
}

impl From<AbstractCallFrame<i64, InterpreterError, ()>> for MockAbstractFrame {
    fn from(frame: AbstractCallFrame<i64, InterpreterError, ()>) -> Self {
        Self::Call(frame)
    }
}

impl From<AbstractDiGraphFrame<i64, InterpreterError, ()>> for MockAbstractFrame {
    fn from(frame: AbstractDiGraphFrame<i64, InterpreterError, ()>) -> Self {
        Self::DiGraph(frame)
    }
}

#[test]
fn abstract_engine_needs_no_concrete_call_lifecycle() {
    assert_dataflow_engine::<AbstractOnlyEngine>();

    // And the abstract frames it drives really do run on it — including
    // `AbstractCallFrame`, whose only engine requirement is `summarize_call`.
    // That is the split's payoff: summarizing a call needs no call convention.
    assert_frame::<
        AbstractOnlyEngine,
        MockAbstractFrame,
        AbstractBlockFrame<i64, InterpreterError, ()>,
    >();
    assert_frame::<
        AbstractOnlyEngine,
        MockAbstractFrame,
        AbstractCallFrame<i64, InterpreterError, ()>,
    >();
    assert_frame::<
        AbstractOnlyEngine,
        MockAbstractFrame,
        AbstractDiGraphFrame<i64, InterpreterError, ()>,
    >();
}

// ===========================================================================
// 4. The umbrellas still work where a universe needs them
// ===========================================================================

/// The narrowing must not cost the umbrella: a total frame enum's engine has to
/// support the union of all its variants, so [`ForwardFrameEngine`] remains the
/// right bound there.
///
/// This needs no instantiation — a generic function body is type-checked at
/// *definition* time, so `needs_all::<I>()` fails to compile the moment
/// `ForwardFrameEngine` stops implying all four components (e.g. if the blanket
/// impl were dropped, or a fifth component added to the umbrella without an
/// impl).
#[allow(dead_code)]
fn umbrella_still_covers_every_component<I: ForwardFrameEngine>() {
    fn needs_all<J>()
    where
        J: StatementDispatch + BlockQueries + DiGraphQueries + CallServices,
    {
    }
    needs_all::<I>();
}

/// Conversely: [`ForwardDataflowFrameEngine`] must keep implying the three
/// traversal components it does extend, so abstract frames can rely on them.
#[allow(dead_code)]
fn dataflow_umbrella_covers_its_three_components<I: ForwardDataflowFrameEngine>() {
    fn needs_traversal<J>()
    where
        J: StatementDispatch + BlockQueries + DiGraphQueries,
    {
    }
    needs_traversal::<I>();
}

// ===========================================================================
// 5. The `*Queries` traits are honestly read-only
// ===========================================================================

/// Implements: [`Interp`], [`BlockQueries`], [`CFGQueries`], [`DiGraphQueries`].
///
/// **Deliberately omits [`Env`]** — it has no SSA storage at all, not even a
/// field for it.
///
/// This is the assertion that keeps the *names* truthful. Each `*Queries` trait
/// requires only `Interp`, so none of their methods can touch the store; the
/// one operation that needs both a query and a write (binding a block's
/// parameters) lives on the crate-private `BlockBinding: Env + BlockQueries`
/// instead. Re-adding `Env` as a `*Queries` supertrait — the obvious way to
/// smuggle a mutating default method back in — stops this engine from compiling.
struct QueriesOnlyEngine;

impl Interp for QueriesOnlyEngine {
    type Value = i64;
    type Error = InterpreterError;
    type Effect = ();
    type Semantics = ForwardEval;

    fn stage(&self) -> CompileStage {
        unimplemented!("type-level mock")
    }
    fn statement(&self) -> Statement {
        unimplemented!("type-level mock")
    }
    fn index(&self) -> EnvIndex {
        unimplemented!("type-level mock")
    }
}

impl BlockQueries for QueriesOnlyEngine {
    fn block_params(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Vec<SSAValue>, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn first_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("type-level mock")
    }
    fn next_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
        _after: Statement,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

impl CFGQueries for QueriesOnlyEngine {
    fn cfg_entry(
        &self,
        _stage: CompileStage,
        _cfg: kirin_ir::CFG,
    ) -> Result<Option<Block>, InterpreterError> {
        unimplemented!("type-level mock")
    }
}

impl DiGraphQueries for QueriesOnlyEngine {}

#[test]
fn query_traits_are_satisfiable_without_env() {
    assert_read_only_queries::<QueriesOnlyEngine>();
}
