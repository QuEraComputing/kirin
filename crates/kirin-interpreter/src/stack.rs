use std::collections::HashSet;
use std::marker::PhantomData;

use kirin_ir::{
    CompileStage, CompileStageInfo, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue,
    SSAValue, SpecializedFunction, Statement,
};

use crate::{
    EvalCall, ConcreteContinuation, ConcreteExt, Continuation, Frame, Interpretable,
    Interpreter, InterpreterError,
};

type StackFrame<V> = Frame<V, Option<Statement>>;

/// Stack-based interpreter that owns execution state and drives evaluation.
///
/// Combines value storage (frames), pipeline reference, and execution logic
/// (step/advance/run/call) in one type. Different interpreter implementations
/// (e.g. [`crate::AbstractInterpreter`]) provide different walking strategies.
///
/// # Error type
///
/// Defaults to [`InterpreterError`]. Users who need additional error variants
/// can define their own error type with `#[from] InterpreterError`:
///
/// ```ignore
/// #[derive(Debug, thiserror::Error)]
/// enum MyError {
///     #[error(transparent)]
///     Interp(#[from] InterpreterError),
///     #[error("division by zero")]
///     DivisionByZero,
/// }
///
/// let mut interp = StackInterpreter::<i64, _, MyError>::new(&pipeline, stage);
/// ```
pub struct StackInterpreter<'ir, V, S, E = InterpreterError, G = ()>
where
    S: CompileStageInfo,
{
    frames: Vec<StackFrame<V>>,
    global: G,
    pipeline: &'ir Pipeline<S>,
    active_stage: CompileStage,
    breakpoints: HashSet<Statement>,
    fuel: Option<u64>,
    max_depth: Option<usize>,
    _error: PhantomData<E>,
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> StackInterpreter<'ir, V, S, E, ()>
where
    S: CompileStageInfo,
{
    pub fn new(pipeline: &'ir Pipeline<S>, active_stage: CompileStage) -> Self {
        Self {
            frames: Vec::new(),
            global: (),
            pipeline,
            active_stage,
            breakpoints: HashSet::default(),
            fuel: None,
            max_depth: None,
            _error: PhantomData,
        }
    }

    /// Attach global state, transforming `G` from `()` to the provided type.
    ///
    /// ```ignore
    /// let interp = StackInterpreter::<i64, _>::new(&pipeline, stage)
    ///     .with_global(MyState::new());
    /// ```
    pub fn with_global<G>(self, global: G) -> StackInterpreter<'ir, V, S, E, G> {
        StackInterpreter {
            frames: self.frames,
            global,
            pipeline: self.pipeline,
            active_stage: self.active_stage,
            breakpoints: self.breakpoints,
            fuel: self.fuel,
            max_depth: self.max_depth,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn global(&self) -> &G {
        &self.global
    }

    pub fn global_mut(&mut self) -> &mut G {
        &mut self.global
    }

    pub fn set_breakpoints(&mut self, stmts: HashSet<Statement>) {
        self.breakpoints = stmts;
    }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Returns the number of times `callee` currently appears on the call stack.
    ///
    /// This enables dialect-specific recursion limits. For example, a dialect
    /// can check the recursion depth and return a default value instead of
    /// recursing further:
    ///
    /// ```ignore
    /// let depth = interp.recursion_depth(self.body_func);
    /// if depth > 100 {
    ///     interp.write(self.result, V::default())?;
    ///     return Ok(Continuation::Continue);
    /// }
    /// Ok(Continuation::Call { callee: self.body_func, args, result: self.result })
    /// ```
    pub fn recursion_depth(&self, callee: SpecializedFunction) -> usize {
        self.frames.iter().filter(|f| f.callee() == callee).count()
    }
}

// -- Frame management (inherent, not on the trait) --------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    pub fn current_frame(&self) -> Result<&StackFrame<V>, E> {
        self.frames
            .last()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub fn current_frame_mut(&mut self) -> Result<&mut StackFrame<V>, E> {
        self.frames
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub fn push_call_frame(&mut self, frame: StackFrame<V>) -> Result<(), E> {
        if let Some(max) = self.max_depth {
            if self.frames.len() >= max {
                return Err(InterpreterError::MaxDepthExceeded.into());
            }
        }
        self.frames.push(frame);
        Ok(())
    }

    pub fn pop_call_frame(&mut self) -> Result<StackFrame<V>, E> {
        self.frames
            .pop()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }
}

// -- Interpreter trait impl -------------------------------------------------

impl<'ir, V, S, E, G> Interpreter<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: CompileStageInfo + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;
    type Ext = ConcreteExt;
    type StageInfo = S;

    fn read_ref(&self, value: SSAValue) -> Result<&V, E> {
        self.current_frame()?
            .read(value)
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.current_frame_mut()?.write(result, value);
        Ok(())
    }

    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), E> {
        self.current_frame_mut()?.write_ssa(ssa, value);
        Ok(())
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.active_stage
    }
}

// -- Call (inherent, not on the trait) --------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: CompileStageInfo + 'ir,
    G: 'ir,
{
    /// Call a specialized function and return its result value.
    pub fn call<L>(&mut self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V>,
        S: HasStageInfo<L>,
    {
        let stage = self.active_stage_info::<L>();
        let spec = callee.expect_info(stage);
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);
        def.eval_call(self, callee, args)
    }
}

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: CompileStageInfo + 'ir,
    G: 'ir,
{
    /// Execute the current statement's dialect semantics.
    /// Returns the raw [`ConcreteContinuation`] without advancing the cursor.
    pub fn step<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        if let Some(ref mut fuel) = self.fuel {
            if *fuel == 0 {
                return Err(InterpreterError::FuelExhausted.into());
            }
            *fuel -= 1;
        }
        let stage = self.active_stage_info::<L>();
        let cursor = self
            .current_frame()?
            .cursor()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let def: &L = cursor.definition(stage);
        def.interpret(self)
    }

    /// Apply cursor mutations for a continuation.
    pub fn advance<L>(&mut self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        match control {
            Continuation::Continue => {
                self.advance_cursor::<L>()?;
            }
            Continuation::Jump(succ, args) => {
                let stage = self.active_stage_info::<L>();
                crate::EvalBlock::bind_block_args(self, stage, succ.target(), args)?;
                let first = succ.target().first_statement(stage);
                self.current_frame_mut()?.set_cursor(first);
            }
            Continuation::Fork(_) => {
                return Err(InterpreterError::UnexpectedControl(
                    "Fork is not supported by concrete interpreters".to_owned(),
                )
                .into());
            }
            Continuation::Call { callee, args, .. } => {
                self.advance_cursor::<L>()?;
                self.push_call_frame_with_args::<L>(*callee, args)?;
            }
            Continuation::Return(_) => {
                self.pop_call_frame()?;
            }
            Continuation::Yield(_) => {
                // No cursor change — the parent op (e.g. eval_block) handles this
            }
            Continuation::Ext(ConcreteExt::Break | ConcreteExt::Halt) => {
                // No cursor change
            }
        }
        Ok(())
    }

    /// Run statements until Return, Halt, or Call.
    /// Ignores breakpoints and Break from dialect intrinsics.
    pub fn run<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        loop {
            let control = self.step::<L>()?;
            match &control {
                Continuation::Continue | Continuation::Jump(..) => {
                    self.advance::<L>(&control)?;
                }
                Continuation::Ext(ConcreteExt::Break) => {
                    self.advance::<L>(&Continuation::Continue)?;
                }
                _ => return Ok(control),
            }
        }
    }

    /// Run statements until a breakpoint, Return, Halt, or Call.
    pub fn run_until_break<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        loop {
            if let Some(cursor) = self.current_frame()?.cursor() {
                if self.breakpoints.contains(&cursor) {
                    return Ok(Continuation::Ext(ConcreteExt::Break));
                }
            }
            let control = self.step::<L>()?;
            match &control {
                Continuation::Continue | Continuation::Jump(..) => {
                    self.advance::<L>(&control)?;
                }
                _ => return Ok(control),
            }
        }
    }

    // -- Internal helpers ---------------------------------------------------

    /// Advance the current frame's cursor past the current statement.
    fn advance_cursor<L>(&mut self) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.active_stage_info::<L>();
        let cursor = self
            .current_frame()?
            .cursor()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let next = *cursor.next::<L>(stage);
        if let Some(next_stmt) = next {
            self.current_frame_mut()?.set_cursor(Some(next_stmt));
        } else {
            // End of block: check for terminator
            let parent_block = *cursor.parent::<L>(stage);
            if let Some(block) = parent_block {
                let term = block.terminator::<L>(stage);
                if term == Some(cursor) {
                    // We just executed the terminator, no next stmt
                    self.current_frame_mut()?.set_cursor(None);
                } else if let Some(t) = term {
                    self.current_frame_mut()?.set_cursor(Some(t));
                } else {
                    self.current_frame_mut()?.set_cursor(None);
                }
            } else {
                self.current_frame_mut()?.set_cursor(None);
            }
        }
        Ok(())
    }

    pub(crate) fn frames_len(&self) -> usize {
        self.frames.len()
    }

    /// Create a new frame for `callee`, bind arguments, and push it.
    ///
    /// Entry block resolution is delegated to the body statement's
    /// [`Interpretable`] impl, which returns `Jump(entry_block, _)`.
    fn push_call_frame_with_args<L>(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        // Delegate entry resolution to the body statement's Interpretable impl
        let entry_block = {
            let stage = self.active_stage_info::<L>();
            let spec = callee.expect_info(stage);
            let body_stmt = *spec.body();
            let def: &L = body_stmt.definition(stage);
            match def.interpret(self)? {
                Continuation::Jump(succ, _) => Some(succ.target()),
                _ => None,
            }
        };
        let stage = self.active_stage_info::<L>();
        let first = entry_block.and_then(|b| b.first_statement(stage));
        self.push_call_frame(Frame::new(callee, first))?;
        if let Some(block) = entry_block {
            crate::EvalBlock::bind_block_args(self, stage, block, args)?;
        }
        Ok(())
    }
}
