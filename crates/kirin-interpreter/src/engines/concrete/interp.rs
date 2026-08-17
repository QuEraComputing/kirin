use std::marker::PhantomData;

use kirin_ir::{
    Block, CFG, CompileStage, Pipeline, Product, SSAValue, StageMeta, Statement, Symbol,
};

use crate::core::query;
use crate::{
    BlockQueries, CFGQueries, CallServices, CallableBody, Callee, Completion, DiGraphQueries, Env,
    EnvIndex, EnvStackStore, ForwardEval, Frame, FunctionTarget, Interp, InterpDispatch,
    InterpLocation, InterpreterError, Linker, SameStageLinker, SparseForwardEffect, StageQuery,
    StatementDispatch, Store, drive_frames,
};

use super::frames::{CallRequest, FrameStackItem};

/// Concrete interpreter mechanism parameterized by one private frame-stack-item type.
///
/// Language crates keep `F` private and expose a domain-specific wrapper, as
/// [`ConcreteInterpreter`] does for the framework-default composition. Member
/// frames stay generic over `F` and never name or construct its variants.
pub struct ConcreteInterpreterCore<'ir, S: StageMeta, V, E, Lk, F> {
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    frames: Vec<F>,
    /// The statement location currently being dispatched, exposed to dialect
    /// rules through [`Interp::stage`]/[`Interp::statement`]/[`Interp::index`].
    location: Option<InterpLocation>,
    _marker: PhantomData<fn() -> E>,
}

type DefaultConcreteCore<'ir, S, V, E, Lk> =
    ConcreteInterpreterCore<'ir, S, V, E, Lk, FrameStackItem<V, E>>;

/// Public concrete interpreter using the framework-default continuation
/// composition (`Block`, `CFG`, `Call`, and `DiGraph`).
///
/// The heterogeneous [`FrameStackItem`] enum is hidden behind this wrapper. A
/// language that adds dialect-owned continuations builds the same kind of
/// wrapper around [`ConcreteInterpreterCore`] with its own private stack-item
/// enum.
pub struct ConcreteInterpreter<'ir, S: StageMeta, V, E, Lk = SameStageLinker> {
    inner: DefaultConcreteCore<'ir, S, V, E, Lk>,
}

impl<'ir, S: StageMeta, V, E> ConcreteInterpreter<'ir, S, V, E, SameStageLinker> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            inner: ConcreteInterpreterCore::new(pipeline),
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk> ConcreteInterpreter<'ir, S, V, E, Lk> {
    pub fn with_linker<Lk2>(self, linker: Lk2) -> ConcreteInterpreter<'ir, S, V, E, Lk2> {
        ConcreteInterpreter {
            inner: self.inner.with_linker(linker),
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.inner.pipeline()
    }
}

impl<'ir, S: StageMeta, V, E, F> ConcreteInterpreterCore<'ir, S, V, E, SameStageLinker, F> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            linker: SameStageLinker,
            store: EnvStackStore::new(),
            frames: Vec::new(),
            location: None,
            _marker: PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk, F> ConcreteInterpreterCore<'ir, S, V, E, Lk, F> {
    /// Swap the calling-convention component (the [`Linker`]).
    pub fn with_linker<Lk2>(self, linker: Lk2) -> ConcreteInterpreterCore<'ir, S, V, E, Lk2, F> {
        ConcreteInterpreterCore {
            pipeline: self.pipeline,
            linker,
            store: self.store,
            frames: self.frames,
            location: self.location,
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S, V, E, Lk, F> Interp for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    type Value = V;
    type Error = E;
    type Effect = SparseForwardEffect<V, F>;
    type Semantics = ForwardEval;

    fn stage(&self) -> CompileStage {
        self.location.expect("interp location not set").stage
    }

    fn statement(&self) -> Statement {
        self.location.expect("interp location not set").statement
    }

    fn index(&self) -> EnvIndex {
        self.location.expect("interp location not set").index
    }
}

impl<'ir, S, V, E, Lk, F> Env for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<V, E> {
        self.store.read(index, value).map_err(E::from)
    }

    fn env_write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), E> {
        self.store.write(index, value, data).map_err(E::from)
    }
}

// The concrete engine provides the whole forward capability surface; it is split
// into one impl block per capability so the components stay individually
// nameable, and the blanket impl gives it `ForwardFrameEngine`/`ForwardFrameEngine`.

impl<'ir, S, V, E, Lk, F> CallServices for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    fn alloc_env(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free_env(&mut self, index: EnvIndex) -> Result<(), E> {
        self.store.free(index).map_err(E::from)
    }

    fn resolve_call(&self, stage: CompileStage, callee: &Callee) -> Result<FunctionTarget, E> {
        self.linker
            .resolve(self.pipeline, stage, callee)
            .map_err(E::from)
    }

    fn enter_function(
        &mut self,
        stage: CompileStage,
        definition: Statement,
        args: Product<V>,
        index: EnvIndex,
    ) -> Result<CallableBody<V>, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let previous = self.location.replace(InterpLocation {
            stage,
            statement: definition,
            index,
        });
        let result = info.dispatch_function_entry(definition, args, self);
        self.location = previous;
        result
    }
}

impl<'ir, S, V, E, Lk, F> StatementDispatch for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<Self::Effect, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let previous = self.location.replace(InterpLocation {
            stage,
            statement,
            index,
        });
        let result = info.dispatch_statement(statement, self);
        self.location = previous;
        result
    }
}

impl<'ir, S, V, E, Lk, F> BlockQueries for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    fn block_params(&self, stage: CompileStage, block: Block) -> Result<Vec<SSAValue>, E> {
        query::block_params(self.pipeline, stage, block).map_err(E::from)
    }

    fn first_statement(&self, stage: CompileStage, block: Block) -> Result<Option<Statement>, E> {
        query::first_statement(self.pipeline, stage, block).map_err(E::from)
    }

    fn next_statement(
        &self,
        stage: CompileStage,
        block: Block,
        after: Statement,
    ) -> Result<Option<Statement>, E> {
        query::next_statement(self.pipeline, stage, block, after).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, F> CFGQueries for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    fn cfg_entry(&self, stage: CompileStage, cfg: CFG) -> Result<Option<Block>, E> {
        query::cfg_entry(self.pipeline, stage, cfg).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, F> DiGraphQueries for ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    fn digraph_walk_plan(
        &self,
        stage: CompileStage,
        graph: kirin_ir::DiGraph,
    ) -> Result<crate::GraphWalkPlan, E> {
        query::digraph_walk_plan(self.pipeline, stage, graph).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, F> ConcreteInterpreterCore<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    F: Frame<Self, F, Completion = Completion<V>> + From<CallRequest<V>>,
{
    /// Resolve `stage`/`function` by name and execute it to completion.
    pub fn call_by_name(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        let stage = self
            .pipeline
            .stage_by_name(stage_name)
            .ok_or_else(|| E::from(InterpreterError::MissingStageName(stage_name.into())))?;
        let function = self
            .pipeline
            .lookup_function_by_name(function_name)
            .ok_or_else(|| E::from(InterpreterError::MissingFunctionName(function_name.into())))?;
        self.call(stage, Callee::Function(function), args)
    }

    /// Resolve a stage-local `symbol` through the linker and execute the
    /// selected callable to completion. This is the convenience form of
    /// [`call`](Self::call) with [`Callee::Named`].
    pub fn call_by_symbol(
        &mut self,
        stage: CompileStage,
        symbol: Symbol,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        self.call(stage, symbol.into(), args)
    }

    /// Execute a function to completion and return its return product.
    ///
    /// The root call is an ordinary [`CallFrame`](crate::CallFrame): the same call boundary
    /// that nested `Call` effects go through owns callee resolution, the
    /// callee activation, body-kind selection, and completion validation —
    /// there is exactly one implementation of that behavior.
    pub fn call(
        &mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        let args: Product<V> = args.into_iter().collect();
        self.frames
            .push(CallRequest::root(stage, callee, args).into());
        self.run()
    }

    /// The generic driver loop: pop the top frame, step it, and apply the
    /// resulting [`FrameEffect`]. `Done`/`Complete` bubble synchronously
    /// through parents until one continues or the stack empties.
    fn run(&mut self) -> Result<Product<V>, E> {
        let mut frames = std::mem::take(&mut self.frames);
        let completion = drive_frames(self, &mut frames);
        self.frames = frames;
        match completion? {
            Completion::Returned(values) => Ok(values),
            Completion::Yielded(_) | Completion::Finished(_) => Err(E::from(
                InterpreterError::Custom("body completion reached the frame-stack root"),
            )),
        }
    }
}

// The bound intentionally mentions the private stack-item type: this is the seam that
// proves the public wrapper's methods are supported without exposing that
// representation in the wrapper's type parameters.
#[allow(private_bounds)]
impl<'ir, S, V, E, Lk> ConcreteInterpreter<'ir, S, V, E, Lk>
where
    S: StageQuery + InterpDispatch<DefaultConcreteCore<'ir, S, V, E, Lk>>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    pub fn call_by_name(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        self.inner.call_by_name(stage_name, function_name, args)
    }

    pub fn call_by_symbol(
        &mut self,
        stage: CompileStage,
        symbol: Symbol,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        self.inner.call_by_symbol(stage, symbol, args)
    }

    pub fn call(
        &mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        self.inner.call(stage, callee, args)
    }
}
