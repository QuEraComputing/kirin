//! Compile-time capability checks. No mock executes IR or stores values.
//!
//! | Engine | Boundary protected |
//! |---|---|
//! | BlockOnlyEngine | Block walking needs no call or graph services |
//! | CallOnlyEngine | Calls need no statement dispatch or forward effects |
//! | AbstractOnlyEngine | Abstract frames need no concrete call lifecycle or CFG queries |
//! | QueriesOnlyEngine | Structural queries need no environment access |
//! | PointAnchoredEngine | Environment access supports program-point anchors |
//!
//! Behavior is covered by `body_kinds` and the engine tests.

#![allow(dead_code)]

use kirin_interpreter::{
    AbstractBlockFrame, AbstractCallFrame, AbstractDiGraphFrame, BlockFrame, BlockQueries,
    CFGFrame, CFGQueries, CallEffect, CallFrame, CallRequest, CallServices, Callee,
    DefaultCallBodyTraversal, DiGraphFrame, DiGraphQueries, Env, EnvIndex,
    ForwardDataflowFrameEngine, ForwardEval, Frame, Interp, InterpreterError, ProgramPoint,
    ResolvedCallable, SSABinding, SparseForwardEffect, StatementDispatch,
};
use kirin_ir::{Block, CompileStage, Product, SSAValue, Statement};

fn assert_frame<I, R, T>()
where
    I: kirin_interpreter::FrameEngine,
    T: Frame<I, R>,
{
}

fn assert_dataflow_engine<I: ForwardDataflowFrameEngine>() {}

fn assert_read_only_queries<I: BlockQueries + CFGQueries + DiGraphQueries>() {}

fn assert_point_env<I: Env<Anchor = ProgramPoint>>() {}

// Type-checked for every SSA environment, without forward-effect bounds.
fn ssa_env_implies_binding<I: Env<Anchor = SSAValue>>() {
    fn requires_binding<T: SSABinding>() {}
    requires_binding::<I>();
}

struct CapabilityChild;

impl From<BlockFrame<i64, InterpreterError>> for CapabilityChild {
    fn from(_: BlockFrame<i64, InterpreterError>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

impl From<CFGFrame<i64, InterpreterError>> for CapabilityChild {
    fn from(_: CFGFrame<i64, InterpreterError>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

impl From<CallRequest<i64>> for CapabilityChild {
    fn from(_: CallRequest<i64>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

impl From<DiGraphFrame<i64, InterpreterError>> for CapabilityChild {
    fn from(_: DiGraphFrame<i64, InterpreterError>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

// Common location stubs; each engine's capabilities remain explicit below.
macro_rules! interp_stub {
    ($engine:ty, $effect:ty) => {
        impl Interp for $engine {
            type Value = i64;
            type Error = InterpreterError;
            type Effect = $effect;
            type Semantics = ForwardEval;

            fn stage(&self) -> CompileStage {
                unimplemented!("compile-time capability test")
            }
            fn statement(&self) -> Statement {
                unimplemented!("compile-time capability test")
            }
            fn index(&self) -> EnvIndex {
                unimplemented!("compile-time capability test")
            }
        }
    };
}

struct BlockOnlyEngine;

interp_stub!(BlockOnlyEngine, SparseForwardEffect<i64, CapabilityChild>);

impl Env for BlockOnlyEngine {
    type Anchor = SSAValue;

    fn env_read(&self, _: EnvIndex, _: SSAValue) -> Result<i64, InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn env_write(&mut self, _: EnvIndex, _: SSAValue, _: i64) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl StatementDispatch for BlockOnlyEngine {
    fn run_statement(
        &mut self,
        _stage: CompileStage,
        _statement: Statement,
        _index: EnvIndex,
    ) -> Result<Self::Effect, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl BlockQueries for BlockOnlyEngine {
    fn block_params(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Vec<SSAValue>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn first_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn next_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
        _after: Statement,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

#[test]
fn block_frame_runs_on_an_engine_with_only_block_queries_and_dispatch() {
    assert_frame::<BlockOnlyEngine, CapabilityChild, BlockFrame<i64, InterpreterError>>();
}

struct CallOnlyEngine;

interp_stub!(CallOnlyEngine, ());

impl Env for CallOnlyEngine {
    type Anchor = SSAValue;

    fn env_read(&self, _: EnvIndex, _: SSAValue) -> Result<i64, InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn env_write(&mut self, _: EnvIndex, _: SSAValue, _: i64) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl CallServices for CallOnlyEngine {
    fn alloc_env(&mut self) -> EnvIndex {
        unimplemented!("compile-time capability test")
    }
    fn free_env(&mut self, _index: EnvIndex) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn resolve_callable(
        &self,
        _stage: CompileStage,
        _callee: &Callee,
    ) -> Result<ResolvedCallable, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

#[test]
fn call_frame_needs_only_call_services_and_ssa_env() {
    assert_frame::<CallOnlyEngine, CapabilityChild, CallFrame<i64, DefaultCallBodyTraversal>>();
}

struct AbstractOnlyEngine;

interp_stub!(AbstractOnlyEngine, SparseForwardEffect<i64, MockAbstractFrame>);

impl Env for AbstractOnlyEngine {
    type Anchor = SSAValue;

    fn env_read(&self, _: EnvIndex, _: SSAValue) -> Result<i64, InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn env_write(&mut self, _: EnvIndex, _: SSAValue, _: i64) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl StatementDispatch for AbstractOnlyEngine {
    fn run_statement(
        &mut self,
        _stage: CompileStage,
        _statement: Statement,
        _index: EnvIndex,
    ) -> Result<Self::Effect, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl BlockQueries for AbstractOnlyEngine {
    fn block_params(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Vec<SSAValue>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn first_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn next_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
        _after: Statement,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl DiGraphQueries for AbstractOnlyEngine {}

impl ForwardDataflowFrameEngine for AbstractOnlyEngine {
    type SummaryKey = ();

    fn analysis_merge(
        &self,
        _current: &Product<i64>,
        _incoming: &Product<i64>,
        _visits: usize,
    ) -> Result<Product<i64>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn contribute_return(&mut self, _values: Product<i64>) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn current_function_key(&self) -> Option<()> {
        unimplemented!("compile-time capability test")
    }

    fn summarize_call(
        &mut self,
        _stage: CompileStage,
        _call: CallEffect<i64>,
        _index: EnvIndex,
    ) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn max_iterations(&self) -> usize {
        unimplemented!("compile-time capability test")
    }
}

struct MockAbstractFrame;

impl From<AbstractBlockFrame<i64, InterpreterError, ()>> for MockAbstractFrame {
    fn from(_: AbstractBlockFrame<i64, InterpreterError, ()>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

impl From<AbstractCallFrame<i64, InterpreterError, ()>> for MockAbstractFrame {
    fn from(_: AbstractCallFrame<i64, InterpreterError, ()>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

impl From<AbstractDiGraphFrame<i64, InterpreterError, ()>> for MockAbstractFrame {
    fn from(_: AbstractDiGraphFrame<i64, InterpreterError, ()>) -> Self {
        unimplemented!("compile-time capability test")
    }
}

#[test]
fn abstract_engine_needs_no_concrete_call_lifecycle() {
    assert_dataflow_engine::<AbstractOnlyEngine>();

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

struct QueriesOnlyEngine;

interp_stub!(QueriesOnlyEngine, ());

impl BlockQueries for QueriesOnlyEngine {
    fn block_params(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Vec<SSAValue>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn first_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
    fn next_statement(
        &self,
        _stage: CompileStage,
        _block: Block,
        _after: Statement,
    ) -> Result<Option<Statement>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl CFGQueries for QueriesOnlyEngine {
    fn cfg_entry(
        &self,
        _stage: CompileStage,
        _cfg: kirin_ir::CFG,
    ) -> Result<Option<Block>, InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

impl DiGraphQueries for QueriesOnlyEngine {}

#[test]
fn query_traits_are_satisfiable_without_env() {
    assert_read_only_queries::<QueriesOnlyEngine>();
}

struct PointAnchoredEngine;

interp_stub!(PointAnchoredEngine, ());

impl Env for PointAnchoredEngine {
    type Anchor = ProgramPoint;

    fn env_read(&self, _: EnvIndex, _: ProgramPoint) -> Result<i64, InterpreterError> {
        unimplemented!("compile-time capability test")
    }

    fn env_write(&mut self, _: EnvIndex, _: ProgramPoint, _: i64) -> Result<(), InterpreterError> {
        unimplemented!("compile-time capability test")
    }
}

#[test]
fn env_access_is_anchor_generic() {
    assert_point_env::<PointAnchoredEngine>();
}
