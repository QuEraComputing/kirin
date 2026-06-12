use kirin_ir::{Block, CompileStage, Pipeline, Product, SSAValue, StageMeta, Statement};

use crate::ctx::EngineEnv;
use crate::{
    Callee, Effect, Env, EnvIndex, EnvStackStore, FunctionTarget, Interp, InterpDispatch,
    InterpreterError, Linker, SameStageLinker, Scope, ScopeBody, ScopeHook, ScopeStep, StageQuery,
    query,
};

/// Concrete executor: runs IR statements over a concrete value domain with an
/// explicit scope stack (no Rust-stack recursion for interpreter control flow).
///
/// ```ignore
/// let mut interp = ConcreteInterpreter::<Stage, i64, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let result = interp.call_by_name("source", "main", [3, 5])?;
/// ```
pub struct ConcreteInterpreter<'ir, S: StageMeta, V, E, Lk = SameStageLinker> {
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    frames: Vec<Frame<V, E>>,
}

enum Frame<V, E> {
    Scope(ScopeFrame<V, E>),
    /// Awaiting a `Return` from the function scope above; binds the returned
    /// product into the caller's activation.
    Call {
        env: EnvIndex,
        results: Product<SSAValue>,
    },
}

struct ScopeFrame<V, E> {
    stage: CompileStage,
    env: EnvIndex,
    owns_env: bool,
    function_boundary: bool,
    /// Entry block of the scope body, for hook-driven re-entry.
    entry_block: Block,
    /// Entry arguments currently bound to the body parameters.
    entry_args: Product<V>,
    block: Block,
    cursor: Option<Statement>,
    results: Product<SSAValue>,
    hook: Option<Box<dyn ScopeHook<V, E>>>,
}

impl<'ir, S: StageMeta, V, E> ConcreteInterpreter<'ir, S, V, E> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            linker: SameStageLinker,
            store: EnvStackStore::new(),
            frames: Vec::new(),
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk> ConcreteInterpreter<'ir, S, V, E, Lk> {
    /// Swap the calling-convention component.
    pub fn with_linker<Lk2>(self, linker: Lk2) -> ConcreteInterpreter<'ir, S, V, E, Lk2> {
        ConcreteInterpreter {
            pipeline: self.pipeline,
            linker,
            store: self.store,
            frames: self.frames,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S, V, E, Lk> Interp for ConcreteInterpreter<'ir, S, V, E, Lk>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    type Value = V;
    type Error = E;

    fn env_read(&self, env: EnvIndex, value: SSAValue) -> Result<V, E> {
        self.store.read(env, value).map_err(E::from)
    }

    fn env_write(&mut self, env: EnvIndex, value: SSAValue, data: V) -> Result<(), E> {
        self.store.write(env, value, data).map_err(E::from)
    }
}

impl<'ir, S, V, E, Lk> ConcreteInterpreter<'ir, S, V, E, Lk>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone,
    E: From<InterpreterError>,
    Lk: Linker<S>,
{
    /// Resolve `stage`/`function` by name and execute.
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
        let target = self
            .linker
            .resolve(self.pipeline, stage, &callee)
            .map_err(E::from)?;
        self.push_function_scope(target, args.into_iter().collect())?;
        self.run()
    }

    fn dispatch(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
    ) -> Result<Effect<V, E>, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        info.dispatch_statement(stage, statement, env, self)
    }

    fn push_function_scope(&mut self, target: FunctionTarget, args: Product<V>) -> Result<(), E> {
        let env = self.store.alloc();
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(target.stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(target.stage)))?;
        let scope = info.dispatch_function_entry(target.stage, target.body, args, env, self)?;
        self.push_scope(target.stage, env, true, true, scope)
    }

    fn push_scope(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        owns_env: bool,
        function_boundary: bool,
        scope: Scope<V, E>,
    ) -> Result<(), E> {
        let entry_block = match scope.body {
            ScopeBody::Block(block) => block,
            ScopeBody::Region(region) => query::region_entry(self.pipeline, stage, region)
                .map_err(E::from)?
                .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?,
            ScopeBody::Immediate => {
                // No body: the scope's args are its results.
                return self.write_results(env, &scope.results, scope.args);
            }
        };
        self.bind_block_args(stage, env, entry_block, &scope.args)?;
        let cursor = query::first_statement(self.pipeline, stage, entry_block).map_err(E::from)?;
        self.frames.push(Frame::Scope(ScopeFrame {
            stage,
            env,
            owns_env,
            function_boundary,
            entry_block,
            entry_args: scope.args,
            block: entry_block,
            cursor,
            results: scope.results,
            hook: scope.hook,
        }));
        Ok(())
    }

    fn bind_block_args(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        block: Block,
        args: &Product<V>,
    ) -> Result<(), E> {
        let params = query::block_params(self.pipeline, stage, block).map_err(E::from)?;
        if params.len() != args.len() {
            return Err(E::from(InterpreterError::BlockArityMismatch {
                block,
                expected: params.len(),
                actual: args.len(),
            }));
        }
        for (param, value) in params.into_iter().zip(args.iter().cloned()) {
            self.env_write(env, param, value)?;
        }
        Ok(())
    }

    fn write_results(
        &mut self,
        env: EnvIndex,
        results: &Product<SSAValue>,
        values: Product<V>,
    ) -> Result<(), E> {
        if results.len() != values.len() {
            return Err(E::from(InterpreterError::ProductArityMismatch {
                expected: results.len(),
                actual: values.len(),
            }));
        }
        for (slot, value) in results.iter().copied().zip(values) {
            self.env_write(env, slot, value)?;
        }
        Ok(())
    }

    fn run(&mut self) -> Result<Product<V>, E> {
        loop {
            // Take the next statement from the top scope frame.
            let (stage, env, block, statement) = {
                let Some(Frame::Scope(frame)) = self.frames.last() else {
                    return Err(E::from(InterpreterError::EmptyFrameStack));
                };
                match frame.cursor {
                    Some(statement) => (frame.stage, frame.env, frame.block, statement),
                    None => {
                        return Err(E::from(if frame.function_boundary {
                            InterpreterError::FunctionBodyFellThrough
                        } else {
                            InterpreterError::BlockFellThrough(frame.block)
                        }));
                    }
                }
            };
            let next =
                query::next_statement(self.pipeline, stage, block, statement).map_err(E::from)?;
            if let Some(Frame::Scope(frame)) = self.frames.last_mut() {
                frame.cursor = next;
            }

            match self.dispatch(stage, statement, env)? {
                Effect::Next => {}
                Effect::Jump(edge) => {
                    self.bind_block_args(stage, env, edge.target, &edge.args)?;
                    let cursor = query::first_statement(self.pipeline, stage, edge.target)
                        .map_err(E::from)?;
                    let Some(Frame::Scope(frame)) = self.frames.last_mut() else {
                        return Err(E::from(InterpreterError::EmptyFrameStack));
                    };
                    frame.block = edge.target;
                    frame.cursor = cursor;
                }
                Effect::Branch(_) | Effect::EnterAny(_) => {
                    return Err(E::from(InterpreterError::IndeterminateBranch));
                }
                Effect::Enter(scope) => {
                    self.push_scope(stage, env, false, false, scope)?;
                }
                Effect::Call(call) => {
                    let resolve_stage = call.stage.unwrap_or(stage);
                    let target = self
                        .linker
                        .resolve(self.pipeline, resolve_stage, &call.callee)
                        .map_err(E::from)?;
                    self.frames.push(Frame::Call {
                        env,
                        results: call.results,
                    });
                    self.push_function_scope(target, call.args)?;
                }
                Effect::Yield(values) => self.apply_yield(values)?,
                Effect::Return(values) => {
                    if let Some(result) = self.apply_return(values)? {
                        return Ok(result);
                    }
                }
            }
        }
    }

    fn apply_yield(&mut self, values: Product<V>) -> Result<(), E> {
        let Some(Frame::Scope(mut frame)) = self.frames.pop() else {
            return Err(E::from(InterpreterError::EmptyFrameStack));
        };
        if frame.function_boundary {
            return Err(E::from(InterpreterError::Custom(
                "yield reached a function boundary",
            )));
        }
        match frame.hook.take() {
            None => self.write_results(frame.env, &frame.results, values),
            Some(hook) => {
                let step = hook.on_yield(
                    &frame.entry_args,
                    values,
                    &mut EngineEnv {
                        interp: self,
                        env: frame.env,
                    },
                )?;
                match step {
                    ScopeStep::Finish(results) => {
                        self.write_results(frame.env, &frame.results, results)
                    }
                    ScopeStep::Repeat { args, hook } => {
                        self.bind_block_args(frame.stage, frame.env, frame.entry_block, &args)?;
                        frame.cursor =
                            query::first_statement(self.pipeline, frame.stage, frame.entry_block)
                                .map_err(E::from)?;
                        frame.block = frame.entry_block;
                        frame.entry_args = args;
                        frame.hook = Some(hook);
                        self.frames.push(Frame::Scope(frame));
                        Ok(())
                    }
                    ScopeStep::RepeatOrFinish { .. } => {
                        Err(E::from(InterpreterError::IndeterminateBranch))
                    }
                }
            }
        }
    }

    /// Unwind to the enclosing function boundary. Returns `Some(values)` when
    /// the returning function was the root invocation.
    fn apply_return(&mut self, values: Product<V>) -> Result<Option<Product<V>>, E> {
        loop {
            match self.frames.pop() {
                Some(Frame::Scope(frame)) if frame.function_boundary => {
                    if frame.owns_env {
                        self.store.free(frame.env).map_err(E::from)?;
                    }
                    break;
                }
                Some(Frame::Scope(_)) => continue,
                Some(Frame::Call { .. }) | None => {
                    return Err(E::from(InterpreterError::Custom(
                        "return without an enclosing function scope",
                    )));
                }
            }
        }
        match self.frames.pop() {
            Some(Frame::Call { env, results }) => {
                self.write_results(env, &results, values)?;
                Ok(None)
            }
            Some(frame) => {
                // Not a call frame: put it back and report the protocol error.
                self.frames.push(frame);
                Err(E::from(InterpreterError::Custom(
                    "function scope without a pending call",
                )))
            }
            None => Ok(Some(values)),
        }
    }
}
