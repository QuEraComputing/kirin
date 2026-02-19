use std::collections::HashSet;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, CompileStageInfo, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue,
    SSAValue, SpecializedFunction, StageInfo, Statement,
};

use crate::{ConcreteControl, Frame, Interpretable, Interpreter, InterpreterError};

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

impl<'ir, V, S, E, G> Interpreter for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    type Value = V;
    type Error = E;
    type Control = ConcreteControl<V>;
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
    V: Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    /// Call a specialized function and return its result value.
    pub fn call<L>(&mut self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        L: Dialect + Interpretable<Self>,
        S: HasStageInfo<L>,
    {
        let initial_depth = self.frames.len();
        let mut pending_results: Vec<ResultValue> = Vec::new();

        self.push_call_frame_with_args::<L>(callee, args)?;

        loop {
            let control = self.run::<L>()?;
            match &control {
                ConcreteControl::Call { result, .. } => pending_results.push(*result),
                ConcreteControl::Halt => {
                    return Err(InterpreterError::UnexpectedControl(
                        "halt during call".to_owned(),
                    )
                    .into());
                }
                ConcreteControl::Return(_) => {}
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected variant during call".to_owned(),
                    )
                    .into());
                }
            }

            let v = match &control {
                ConcreteControl::Return(v) => Some(v.clone()),
                _ => None,
            };

            self.advance::<L>(&control)?;

            if let Some(v) = v {
                if self.frames.len() == initial_depth {
                    return Ok(v);
                }
                let result = pending_results
                    .pop()
                    .ok_or_else(|| InterpreterError::NoFrame.into())?;
                self.write(result, v)?;
            }
        }
    }
}

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    /// Execute the current statement's dialect semantics.
    /// Returns the raw [`ConcreteControl`] without advancing the cursor.
    pub fn step<L>(&mut self) -> Result<ConcreteControl<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        if let Some(ref mut fuel) = self.fuel {
            if *fuel == 0 {
                return Err(InterpreterError::FuelExhausted.into());
            }
            *fuel -= 1;
        }
        let stage = self.resolve_stage::<L>();
        let cursor = self
            .current_frame()?
            .cursor()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let def: &L = cursor.definition(stage);
        def.interpret(self)
    }

    /// Apply cursor mutations for a control action.
    pub fn advance<L>(&mut self, control: &ConcreteControl<V>) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        match control {
            ConcreteControl::Continue => {
                self.advance_cursor::<L>()?;
            }
            ConcreteControl::Jump(block, args) => {
                self.bind_block_args::<L>(*block, args)?;
                let first = self.first_stmt_in_block::<L>(*block);
                self.current_frame_mut()?.set_cursor(first);
            }
            ConcreteControl::Call { callee, args, .. } => {
                self.advance_cursor::<L>()?;
                self.push_call_frame_with_args::<L>(*callee, args)?;
            }
            ConcreteControl::Return(_) => {
                self.pop_call_frame()?;
            }
            ConcreteControl::Break | ConcreteControl::Halt => {
                // No cursor change
            }
        }
        Ok(())
    }

    /// Run statements until Return, Halt, or Call.
    /// Ignores breakpoints and Break from dialect intrinsics.
    pub fn run<L>(&mut self) -> Result<ConcreteControl<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        loop {
            let control = self.step::<L>()?;
            match &control {
                ConcreteControl::Continue | ConcreteControl::Jump(..) => {
                    self.advance::<L>(&control)?;
                }
                ConcreteControl::Break => {
                    self.advance::<L>(&ConcreteControl::Continue)?;
                }
                _ => return Ok(control),
            }
        }
    }

    /// Run statements until a breakpoint, Return, Halt, or Call.
    pub fn run_until_break<L>(&mut self) -> Result<ConcreteControl<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        loop {
            if let Some(cursor) = self.current_frame()?.cursor() {
                if self.breakpoints.contains(&cursor) {
                    return Ok(ConcreteControl::Break);
                }
            }
            let control = self.step::<L>()?;
            match &control {
                ConcreteControl::Continue | ConcreteControl::Jump(..) => {
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
        let stage = self.resolve_stage::<L>();
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

    fn resolve_stage<L>(&self) -> &'ir StageInfo<L>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        self.pipeline
            .stage(self.active_stage)
            .and_then(|s| s.try_stage_info())
            .expect("active stage does not contain StageInfo for this dialect")
    }

    fn resolve_entry<L>(&self, callee: SpecializedFunction) -> Option<Statement>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.resolve_stage::<L>();
        let spec = callee.expect_info(stage);
        let body_stmt = *spec.body();
        let region = body_stmt.regions::<L>(stage).next()?;
        let block = region.blocks(stage).next()?;
        self.first_stmt_in_block::<L>(block)
    }

    fn first_stmt_in_block<L>(&self, block: Block) -> Option<Statement>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.resolve_stage::<L>();
        let block_info = block.expect_info(stage);
        if let Some(&head) = block_info.statements.head() {
            Some(head)
        } else {
            block_info.terminator
        }
    }

    fn bind_block_args<L>(&mut self, block: Block, args: &[V]) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.resolve_stage::<L>();
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }
        for (ba, val) in block_info.arguments.iter().zip(args.iter()) {
            self.current_frame_mut()?
                .write_ssa(SSAValue::from(*ba), val.clone());
        }
        Ok(())
    }

    /// Create a new frame for `callee`, bind arguments, and push it.
    fn push_call_frame_with_args<L>(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let entry = self.resolve_entry::<L>(callee);
        let mut frame = Frame::new(callee, entry);
        if let Some(entry_stmt) = entry {
            let stage = self.resolve_stage::<L>();
            let parent_block = *entry_stmt.parent::<L>(stage);
            if let Some(block) = parent_block {
                let block_info = block.expect_info(stage);
                if block_info.arguments.len() != args.len() {
                    return Err(InterpreterError::ArityMismatch {
                        expected: block_info.arguments.len(),
                        got: args.len(),
                    }
                    .into());
                }
                for (ba, val) in block_info.arguments.iter().zip(args.iter()) {
                    frame.write_ssa(SSAValue::from(*ba), val.clone());
                }
            }
        }
        self.push_call_frame(frame)?;
        Ok(())
    }
}
