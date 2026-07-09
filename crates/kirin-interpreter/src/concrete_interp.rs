use std::marker::PhantomData;

use kirin_ir::{Block, CompileStage, Pipeline, Product, Region, SSAValue, StageMeta, Statement};

use crate::{
    BodyFrame, Callee, Completion, Env, EnvIndex, EnvStackStore, ForwardEffect, ForwardEval, Frame,
    FrameBuild, FrameDriver, FunctionBody, FunctionTarget, Interp, InterpDispatch, InterpLocation,
    InterpreterError, Linker, SameStageLinker, StageQuery, StandardFrame, Store, drive_frames,
    query,
};

/// Concrete executor: runs IR over a concrete value domain with an explicit
/// frame stack (no Rust-stack recursion for interpreter control flow).
///
/// Traversal lives in [`Frame`]s, not in the engine: the driver pops a frame,
/// steps it, and applies the returned [`FrameEffect`](crate::FrameEffect). The total frame type `F`
/// defaults to [`StandardFrame`]; a compiler author can supply a custom frame
/// enum — reusing the standard frames via [`FrameBuild`] — to customize
/// traversal without forking the engine.
///
/// ```ignore
/// let mut interp = ConcreteInterpreter::<Stage, i64, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let result = interp.call_by_name("source", "main", [3, 5])?;
/// ```
pub struct ConcreteInterpreter<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    F = StandardFrame<V, E>,
> {
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    frames: Vec<F>,
    /// The statement location currently being dispatched, exposed to dialect
    /// rules through [`Interp::stage`]/[`Interp::statement`]/[`Interp::index`].
    location: Option<InterpLocation>,
    _marker: PhantomData<fn() -> E>,
}

impl<'ir, S: StageMeta, V, E, F> ConcreteInterpreter<'ir, S, V, E, SameStageLinker, F> {
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

impl<'ir, S: StageMeta, V, E, Lk, F> ConcreteInterpreter<'ir, S, V, E, Lk, F> {
    /// Swap the calling-convention component (the [`Linker`]).
    pub fn with_linker<Lk2>(self, linker: Lk2) -> ConcreteInterpreter<'ir, S, V, E, Lk2, F> {
        ConcreteInterpreter {
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

impl<'ir, S, V, E, Lk, F> Interp for ConcreteInterpreter<'ir, S, V, E, Lk, F>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    type Value = V;
    type Error = E;
    type Effect = ForwardEffect<V, F>;
    type Kind = ForwardEval;

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

impl<'ir, S, V, E, Lk, F> Env for ConcreteInterpreter<'ir, S, V, E, Lk, F>
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

impl<'ir, S, V, E, Lk, F> FrameDriver for ConcreteInterpreter<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self, ForwardEval>,
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

    fn enter_function(
        &mut self,
        stage: CompileStage,
        body: Statement,
        args: Product<V>,
        index: EnvIndex,
    ) -> Result<FunctionBody<V>, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let previous = self.location.replace(InterpLocation {
            stage,
            statement: body,
            index,
        });
        let result = info.dispatch_function_entry(body, args, self);
        self.location = previous;
        result
    }

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

    fn region_entry(&self, stage: CompileStage, region: Region) -> Result<Option<Block>, E> {
        query::region_entry(self.pipeline, stage, region).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk, F> ConcreteInterpreter<'ir, S, V, E, Lk, F>
where
    S: StageQuery + InterpDispatch<Self, ForwardEval>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    F: Frame<Self, Completion = Completion<V>> + FrameBuild<V, E>,
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

    /// Execute a function to completion and return its return product.
    pub fn call(
        &mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        let target = self.resolve_call(stage, &callee)?;
        let index = self.alloc_env();
        let args: Product<V> = args.into_iter().collect();
        let body = self.enter_function(target.stage, target.body, args, index)?;
        let frame = BodyFrame::function(self, target.stage, index, body.region, body.args)?;
        self.frames.push(F::from_body(frame));
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
            Completion::Finished(_) => Err(E::from(InterpreterError::Custom(
                "body completion reached the frame-stack root",
            ))),
        }
    }
}
