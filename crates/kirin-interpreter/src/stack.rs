use std::collections::HashSet;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageAction, StageInfo, StageMeta, Statement, SupportsStageDispatch,
};

use crate::{
    ConcreteContinuation, ConcreteExt, Continuation, EvalCall, Frame, Interpretable, Interpreter,
    InterpreterError,
};

struct StackFrameExtra<'ir, V, S, E, G>
where
    S: StageMeta,
{
    cursor: Option<Statement>,
    dispatch: DynFrameDispatch<'ir, V, S, E, G>,
}

type StackFrame<'ir, V, S, E, G> = Frame<V, StackFrameExtra<'ir, V, S, E, G>>;

struct StageDispatchTable<'ir, V, S, E, G>
where
    S: StageMeta,
{
    by_stage: Vec<Option<DynFrameDispatch<'ir, V, S, E, G>>>,
}

type DynStepFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>) -> Result<ConcreteContinuation<V>, E>;
type DynAdvanceFn<'ir, V, S, E, G> =
    fn(&mut StackInterpreter<'ir, V, S, E, G>, &ConcreteContinuation<V>) -> Result<(), E>;

#[doc(hidden)]
pub struct DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    step: DynStepFn<'ir, V, S, E, G>,
    advance: DynAdvanceFn<'ir, V, S, E, G>,
}

impl<'ir, V, S, E, G> Copy for DynFrameDispatch<'ir, V, S, E, G> where S: StageMeta {}

impl<'ir, V, S, E, G> Clone for DynFrameDispatch<'ir, V, S, E, G>
where
    S: StageMeta,
{
    fn clone(&self) -> Self {
        *self
    }
}

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
    S: StageMeta,
{
    call_stack: Vec<StackFrame<'ir, V, S, E, G>>,
    dispatch_table: StageDispatchTable<'ir, V, S, E, G>,
    global: G,
    pipeline: &'ir Pipeline<S>,
    root_stage: CompileStage,
    breakpoints: HashSet<Statement>,
    fuel: Option<u64>,
    max_depth: Option<usize>,
    _error: PhantomData<E>,
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> StackInterpreter<'ir, V, S, E, ()>
where
    V: Clone + 'ir,
    S: StageMeta,
    E: From<InterpreterError> + 'ir,
    S: SupportsStageDispatch<
            FrameDispatchAction<'ir, V, S, E, ()>,
            DynFrameDispatch<'ir, V, S, E, ()>,
            E,
        >,
{
    /// Create a stack interpreter with unit global state.
    ///
    /// The interpreter is rooted at `stage` when no call frame is active.
    /// Per-stage dynamic dispatch is precomputed from `pipeline`.
    pub fn new(pipeline: &'ir Pipeline<S>, stage: CompileStage) -> Self {
        Self::new_with_global(pipeline, stage, ())
    }
}

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    S: StageMeta,
    E: From<InterpreterError> + 'ir,
    S: SupportsStageDispatch<
            FrameDispatchAction<'ir, V, S, E, G>,
            DynFrameDispatch<'ir, V, S, E, G>,
            E,
        >,
    G: 'ir,
{
    /// Create a stack interpreter with explicit global state.
    ///
    /// The interpreter is rooted at `stage` when no call frame is active.
    /// Per-stage dynamic dispatch is precomputed from `pipeline`.
    pub fn new_with_global(pipeline: &'ir Pipeline<S>, stage: CompileStage, global: G) -> Self {
        let dispatch_table = Self::build_dispatch_table(pipeline);
        Self {
            call_stack: Vec::new(),
            dispatch_table,
            global,
            pipeline,
            root_stage: stage,
            breakpoints: HashSet::default(),
            fuel: None,
            max_depth: None,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    S: StageMeta,
{
    /// Set an instruction budget for execution.
    ///
    /// Each executed statement consumes one unit. Exceeding the budget
    /// returns [`InterpreterError::FuelExhausted`].
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    /// Set the maximum call-frame depth.
    ///
    /// Pushing beyond this limit returns [`InterpreterError::MaxDepthExceeded`].
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

#[doc(hidden)]
pub struct CallDynAction<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    interp: &'a mut StackInterpreter<'ir, V, S, E, G>,
    callee: SpecializedFunction,
    args: &'a [V],
}

impl<'a, 'ir, V, S, E, G, L> StageAction<S, L> for CallDynAction<'a, 'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L>
        + EvalCall<'ir, StackInterpreter<'ir, V, S, E, G>, L, Result = V>
        + 'ir,
{
    type Output = V;
    type Error = E;

    fn run(
        &mut self,
        stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        self.interp
            .call_with_stage_id::<L>(self.callee, stage_id, self.args)
    }
}

fn dyn_step_for_lang<'ir, V, S, E, G, L>(
    interp: &mut StackInterpreter<'ir, V, S, E, G>,
) -> Result<ConcreteContinuation<V>, E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    let stage_id = interp.current_frame_stage()?;
    interp.step_with_stage_id::<L>(stage_id)
}

fn dyn_advance_for_lang<'ir, V, S, E, G, L>(
    interp: &mut StackInterpreter<'ir, V, S, E, G>,
    control: &ConcreteContinuation<V>,
) -> Result<(), E>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    let stage_id = interp.current_frame_stage()?;
    interp.advance_frame_with_stage_id::<L>(stage_id, control)
}

#[doc(hidden)]
pub struct FrameDispatchAction<'ir, V, S, E, G>
where
    S: StageMeta,
{
    marker: PhantomData<(&'ir (), V, S, E, G)>,
}

impl<'ir, V, S, E, G, L> StageAction<S, L> for FrameDispatchAction<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Output = DynFrameDispatch<'ir, V, S, E, G>;
    type Error = E;

    fn run(
        &mut self,
        _stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(DynFrameDispatch {
            step: dyn_step_for_lang::<V, S, E, G, L>,
            advance: dyn_advance_for_lang::<V, S, E, G, L>,
        })
    }
}

#[doc(hidden)]
pub struct PushCallFrameDynAction<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    interp: &'a mut StackInterpreter<'ir, V, S, E, G>,
    callee: SpecializedFunction,
    args: &'a [V],
}

impl<'a, 'ir, V, S, E, G, L> StageAction<S, L> for PushCallFrameDynAction<'a, 'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    S: SupportsStageDispatch<
            FrameDispatchAction<'ir, V, S, E, G>,
            DynFrameDispatch<'ir, V, S, E, G>,
            E,
        >,
    G: 'ir,
    L: Dialect + Interpretable<'ir, StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Output = ();
    type Error = E;

    fn run(
        &mut self,
        stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let stage = self.interp.resolve_stage_info::<L>(stage_id)?;
        self.interp
            .push_call_frame_in_resolved_stage::<L>(self.callee, stage_id, stage, self.args)
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    S: StageMeta,
{
    /// Borrow immutable interpreter-global state.
    pub fn global(&self) -> &G {
        &self.global
    }

    /// Borrow mutable interpreter-global state.
    pub fn global_mut(&mut self) -> &mut G {
        &mut self.global
    }

    /// Replace the current breakpoint set.
    ///
    /// Breakpoints are only observed by `run_until_break*` entrypoints.
    pub fn set_breakpoints(&mut self, stmts: HashSet<Statement>) {
        self.breakpoints = stmts;
    }

    /// Clear all configured breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }
}

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    E: From<InterpreterError>,
    S: StageMeta,
{
    fn build_dispatch_table(pipeline: &'ir Pipeline<S>) -> StageDispatchTable<'ir, V, S, E, G>
    where
        V: Clone + 'ir,
        E: 'ir,
        S: 'ir
            + SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        G: 'ir,
    {
        let mut by_stage = Vec::with_capacity(pipeline.stages().len());
        for stage in pipeline.stages() {
            let dispatch = stage.stage_id().and_then(|stage_id| {
                Self::resolve_dispatch_for_stage_in_pipeline(pipeline, stage_id).ok()
            });
            by_stage.push(dispatch);
        }
        StageDispatchTable { by_stage }
    }

    fn current_frame_ref(&self) -> Result<&StackFrame<'ir, V, S, E, G>, E> {
        self.call_stack
            .last()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    fn current_frame_mut_ref(&mut self) -> Result<&mut StackFrame<'ir, V, S, E, G>, E> {
        self.call_stack
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub(crate) fn current_cursor(&self) -> Result<Option<Statement>, E> {
        Ok(self.current_frame_ref()?.extra().cursor)
    }

    pub(crate) fn set_current_cursor(&mut self, cursor: Option<Statement>) -> Result<(), E> {
        self.current_frame_mut_ref()?.extra_mut().cursor = cursor;
        Ok(())
    }

    fn active_stage_from_frames(&self) -> CompileStage {
        self.call_stack
            .last()
            .map(Frame::stage)
            .unwrap_or(self.root_stage)
    }

    fn read_ref_from_current_frame(&self, value: SSAValue) -> Result<&V, E> {
        let frame = self.current_frame_ref()?;
        frame
            .read(value)
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    fn write_to_current_frame(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.current_frame_mut_ref()?.write(result, value);
        Ok(())
    }

    fn write_ssa_to_current_frame(&mut self, ssa: SSAValue, value: V) -> Result<(), E> {
        self.current_frame_mut_ref()?.write_ssa(ssa, value);
        Ok(())
    }

    pub(crate) fn frame_depth(&self) -> usize {
        self.call_stack.len()
    }

    fn public_frame_to_internal(
        frame: Frame<V, Option<Statement>>,
        dispatch: DynFrameDispatch<'ir, V, S, E, G>,
    ) -> StackFrame<'ir, V, S, E, G> {
        let (callee, stage, values, cursor) = frame.into_parts();
        Frame::with_values(callee, stage, values, StackFrameExtra { cursor, dispatch })
    }

    fn internal_frame_to_public(frame: StackFrame<'ir, V, S, E, G>) -> Frame<V, Option<Statement>> {
        let (callee, stage, values, extra) = frame.into_parts();
        Frame::with_values(callee, stage, values, extra.cursor)
    }

    /// Push a call frame and eagerly resolve per-frame dynamic dispatch from
    /// `frame.stage()`. Fails atomically when depth or stage dispatch checks fail.
    pub fn push_frame(&mut self, frame: Frame<V, Option<Statement>>) -> Result<(), E>
    where
        V: Clone + 'ir,
        E: 'ir,
        S: 'ir
            + SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        G: 'ir,
    {
        if let Some(max) = self.max_depth {
            if self.call_stack.len() >= max {
                return Err(InterpreterError::MaxDepthExceeded.into());
            }
        }
        let dispatch = self.resolve_dispatch_for_stage(frame.stage())?;
        let internal = Self::public_frame_to_internal(frame, dispatch);
        self.call_stack.push(internal);
        Ok(())
    }
}

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    E: From<InterpreterError>,
    S: StageMeta,
{
    /// Pop the current call frame and its paired dynamic dispatch entry.
    pub fn pop_frame(&mut self) -> Result<Frame<V, Option<Statement>>, E> {
        let frame = self
            .call_stack
            .pop()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        Ok(Self::internal_frame_to_public(frame))
    }
}

// -- Interpreter trait impl -------------------------------------------------

impl<'ir, V, S, E, G> Interpreter<'ir> for StackInterpreter<'ir, V, S, E, G>
where
    V: 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;
    type Ext = ConcreteExt;
    type StageInfo = S;

    fn read_ref(&self, value: SSAValue) -> Result<&V, E> {
        self.read_ref_from_current_frame(value)
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.write_to_current_frame(result, value)
    }

    fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Result<(), E> {
        self.write_ssa_to_current_frame(ssa, value)
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.active_stage_from_frames()
    }
}

// -- Call (inherent, not on the trait) --------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Call a specialized function and return its result value using strict
    /// typed-stage checking.
    pub fn call_in_stage<L>(&mut self, callee: SpecializedFunction, args: &[V]) -> Result<V, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V> + 'ir,
    {
        let stage_id = self.active_stage();
        self.call_with_stage_id::<L>(callee, stage_id, args)
    }

    /// Stage-dynamic call entrypoint. The target dialect is resolved at
    /// runtime from stage metadata.
    pub fn call(
        &mut self,
        callee: SpecializedFunction,
        stage: CompileStage,
        args: &[V],
    ) -> Result<V, E>
    where
        for<'a> S: SupportsStageDispatch<CallDynAction<'a, 'ir, V, S, E, G>, V, E>,
    {
        let pipeline = self.pipeline;
        let mut action = CallDynAction {
            interp: self,
            callee,
            args,
        };
        Self::dispatch_in_pipeline(pipeline, stage, &mut action)
    }
}

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Execute the current statement's dialect semantics.
    /// Returns the raw [`ConcreteContinuation`] without advancing the cursor.
    pub fn step_in_stage<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage_id = self.current_frame_stage()?;
        self.step_with_stage_id::<L>(stage_id)
    }

    /// Stage-dynamic entrypoint.
    pub fn step(&mut self) -> Result<ConcreteContinuation<V>, E> {
        let dispatch = self.current_frame_dispatch()?;
        (dispatch.step)(self)
    }

    /// Apply cursor mutations for a continuation with strict typed-stage
    /// checking on the current frame stage.
    pub fn advance_in_stage<L>(&mut self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage_id = self.current_frame_stage()?;
        self.advance_frame_with_stage_id::<L>(stage_id, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.push_call_frame_with_args(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Stage-dynamic entrypoint.
    pub fn advance(&mut self, control: &ConcreteContinuation<V>) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        let dispatch = self.current_frame_dispatch()?;
        (dispatch.advance)(self, control)?;
        if let Continuation::Call {
            callee,
            stage: callee_stage,
            args,
            ..
        } = control
        {
            self.push_call_frame_with_args(*callee, *callee_stage, args)?;
        }
        Ok(())
    }

    /// Run statements until Return, Halt, or Call.
    /// Ignores breakpoints and Break from dialect intrinsics.
    pub fn run_in_stage<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        self.drive_loop(
            false,
            true,
            |interp| interp.step_in_stage::<L>(),
            |interp, control| interp.advance_in_stage::<L>(control),
        )
    }

    /// Stage-dynamic entrypoint.
    pub fn run(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        self.drive_loop(
            false,
            true,
            |interp| interp.step(),
            |interp, control| interp.advance(control),
        )
    }

    /// Run statements until a breakpoint, Return, Halt, or Call.
    pub fn run_until_break_in_stage<L>(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        self.drive_loop(
            true,
            false,
            |interp| interp.step_in_stage::<L>(),
            |interp, control| interp.advance_in_stage::<L>(control),
        )
    }

    /// Stage-dynamic entrypoint.
    pub fn run_until_break(&mut self) -> Result<ConcreteContinuation<V>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        self.drive_loop(
            true,
            false,
            |interp| interp.step(),
            |interp, control| interp.advance(control),
        )
    }

    // -- Internal helpers ---------------------------------------------------

    fn drive_loop<Step, Advance>(
        &mut self,
        stop_on_breakpoint: bool,
        swallow_break: bool,
        mut step_fn: Step,
        mut advance_fn: Advance,
    ) -> Result<ConcreteContinuation<V>, E>
    where
        Step: FnMut(&mut Self) -> Result<ConcreteContinuation<V>, E>,
        Advance: FnMut(&mut Self, &ConcreteContinuation<V>) -> Result<(), E>,
    {
        loop {
            if stop_on_breakpoint {
                if let Some(cursor) = self.current_cursor()? {
                    if self.breakpoints.contains(&cursor) {
                        return Ok(Continuation::Ext(ConcreteExt::Break));
                    }
                }
            }

            let control = step_fn(self)?;
            match &control {
                Continuation::Continue | Continuation::Jump(..) => advance_fn(self, &control)?,
                Continuation::Ext(ConcreteExt::Break) if swallow_break => {
                    advance_fn(self, &Continuation::Continue)?
                }
                _ => return Ok(control),
            }
        }
    }

    fn current_frame_stage(&self) -> Result<CompileStage, E> {
        Ok(self.current_frame_ref()?.stage())
    }

    fn current_frame_dispatch(&self) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E> {
        Ok(self.current_frame_ref()?.extra().dispatch)
    }

    fn resolve_dispatch_for_stage_in_pipeline(
        pipeline: &'ir Pipeline<S>,
        stage_id: CompileStage,
    ) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
    {
        let mut action = FrameDispatchAction {
            marker: PhantomData,
        };
        Self::dispatch_in_pipeline(pipeline, stage_id, &mut action)
    }

    fn resolve_dispatch_for_stage(
        &self,
        stage_id: CompileStage,
    ) -> Result<DynFrameDispatch<'ir, V, S, E, G>, E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
    {
        let idx = kirin_ir::Id::from(stage_id).raw();
        match self.dispatch_table.by_stage.get(idx).copied().flatten() {
            Some(dispatch) => Ok(dispatch),
            None => {
                if self.pipeline.stage(stage_id).is_none() {
                    Err(InterpreterError::MissingStage { stage: stage_id }.into())
                } else {
                    Err(InterpreterError::MissingStageDialect { stage: stage_id }.into())
                }
            }
        }
    }

    fn spend_fuel(&mut self) -> Result<(), E> {
        if let Some(ref mut fuel) = self.fuel {
            if *fuel == 0 {
                return Err(InterpreterError::FuelExhausted.into());
            }
            *fuel -= 1;
        }
        Ok(())
    }

    fn bind_block_args_in_stage<L>(
        &mut self,
        stage: &StageInfo<L>,
        block: Block,
        args: &[V],
    ) -> Result<(), E>
    where
        L: Dialect,
    {
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }
        let arg_ssas: Vec<SSAValue> = block_info
            .arguments
            .iter()
            .map(|ba| SSAValue::from(*ba))
            .collect();
        for (ssa, val) in arg_ssas.iter().zip(args.iter()) {
            self.write_ssa(*ssa, val.clone())?;
        }
        Ok(())
    }

    fn call_with_stage_id<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: &[V],
    ) -> Result<V, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V> + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;
        self.call_in_resolved_stage::<L>(callee, stage_id, stage, args)
    }

    fn call_in_resolved_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<V, E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + EvalCall<'ir, Self, L, Result = V> + 'ir,
    {
        let spec =
            callee
                .get_info(stage)
                .ok_or_else(|| InterpreterError::MissingCalleeAtStage {
                    callee,
                    stage: stage_id,
                })?;
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);
        def.eval_call(self, stage, callee, args)
    }

    fn step_with_stage_id<L>(
        &mut self,
        stage_id: CompileStage,
    ) -> Result<ConcreteContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;
        self.step_in_resolved_stage::<L>(stage_id, stage)
    }

    fn step_in_resolved_stage<L>(
        &mut self,
        _stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
    ) -> Result<ConcreteContinuation<V>, E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        self.spend_fuel()?;
        let cursor = self
            .current_cursor()?
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let def: &L = cursor.definition(stage);
        def.interpret(self)
    }

    fn advance_frame_with_stage_id<L>(
        &mut self,
        stage_id: CompileStage,
        control: &ConcreteContinuation<V>,
    ) -> Result<(), E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;
        self.advance_frame_in_resolved_stage::<L>(stage_id, stage, control)
    }

    fn advance_frame_in_resolved_stage<L>(
        &mut self,
        _stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
        control: &ConcreteContinuation<V>,
    ) -> Result<(), E>
    where
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        match control {
            Continuation::Continue => {
                self.advance_cursor_in_stage::<L>(stage)?;
            }
            Continuation::Jump(succ, args) => {
                self.bind_block_args_in_stage::<L>(stage, succ.target(), args)?;
                let first = succ.target().first_statement(stage);
                self.set_current_cursor(first)?;
            }
            Continuation::Fork(_) => {
                return Err(InterpreterError::UnexpectedControl(
                    "Fork is not supported by concrete interpreters".to_owned(),
                )
                .into());
            }
            Continuation::Call { .. } => {
                self.advance_cursor_in_stage::<L>(stage)?;
            }
            Continuation::Return(_) => {
                self.pop_frame()?;
            }
            Continuation::Yield(_) => {}
            Continuation::Ext(ConcreteExt::Break | ConcreteExt::Halt) => {}
        }
        Ok(())
    }

    fn advance_cursor_in_stage<L>(&mut self, stage: &StageInfo<L>) -> Result<(), E>
    where
        L: Dialect,
    {
        let cursor = self
            .current_cursor()?
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        let next = *cursor.next::<L>(stage);
        if let Some(next_stmt) = next {
            self.set_current_cursor(Some(next_stmt))?;
        } else {
            let parent_block = *cursor.parent::<L>(stage);
            if let Some(block) = parent_block {
                let term = block.terminator::<L>(stage);
                if term == Some(cursor) {
                    self.set_current_cursor(None)?;
                } else if let Some(t) = term {
                    self.set_current_cursor(Some(t))?;
                } else {
                    self.set_current_cursor(None)?;
                }
            } else {
                self.set_current_cursor(None)?;
            }
        }
        Ok(())
    }

    fn push_call_frame_with_args(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: &[V],
    ) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        for<'a> S: SupportsStageDispatch<PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    {
        let pipeline = self.pipeline;
        let mut action = PushCallFrameDynAction {
            interp: self,
            callee,
            args,
        };
        Self::dispatch_in_pipeline(pipeline, stage_id, &mut action)
    }

    fn push_call_frame_in_resolved_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<(), E>
    where
        S: SupportsStageDispatch<
                FrameDispatchAction<'ir, V, S, E, G>,
                DynFrameDispatch<'ir, V, S, E, G>,
                E,
            >,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let spec =
            callee
                .get_info(stage)
                .ok_or_else(|| InterpreterError::MissingCalleeAtStage {
                    callee,
                    stage: stage_id,
                })?;
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);

        // Push callee frame first so `active_stage()` during body interpretation
        // resolves to the callee stage without mutable stage tracking on the
        // interpreter state.
        self.push_frame(Frame::new(callee, stage_id, None))?;
        let entry_block = match def.interpret(self) {
            Ok(Continuation::Jump(succ, _)) => succ.target(),
            Ok(_) => {
                let _ = self.pop_frame();
                return Err(InterpreterError::MissingEntry.into());
            }
            Err(err) => {
                let _ = self.pop_frame();
                return Err(err);
            }
        };

        let first = entry_block.first_statement(stage);
        self.set_current_cursor(first)?;
        if let Err(err) = self.bind_block_args_in_stage::<L>(stage, entry_block, args) {
            let _ = self.pop_frame();
            return Err(err);
        }
        Ok(())
    }
}
