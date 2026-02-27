use std::marker::PhantomData;

use kirin_ir::{CompileStage, Pipeline, ResultValue, SSAValue, SpecializedFunction, StageMeta};
use rustc_hash::FxHashMap;

use super::{FixpointState, SummaryCache};
use crate::result::AnalysisResult;
use crate::widening::WideningStrategy;
use crate::{AbstractValue, Frame, Interpreter, InterpreterError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SummaryKey {
    pub(crate) stage: CompileStage,
    pub(crate) callee: SpecializedFunction,
}

/// Worklist-based abstract interpreter for fixpoint computation.
///
/// Unlike [`crate::StackInterpreter`] which follows a single concrete execution
/// path, `AbstractInterpreter` explores all reachable paths by joining abstract
/// states at block entry points and iterating until a fixpoint is reached.
///
/// Widening is applied at join points to guarantee termination for infinite
/// abstract domains.
pub struct AbstractInterpreter<'ir, V, S, E = InterpreterError, G = ()>
where
    S: StageMeta,
{
    pub(crate) pipeline: &'ir Pipeline<S>,
    pub(crate) root_stage: CompileStage,
    pub(crate) global: G,
    pub(crate) frames: Vec<Frame<V, FixpointState>>,
    pub(crate) widening_strategy: WideningStrategy,
    pub(crate) max_iterations: usize,
    pub(crate) narrowing_iterations: usize,
    pub(crate) summaries: FxHashMap<SummaryKey, SummaryCache<V>>,
    pub(crate) max_depth: Option<usize>,
    pub(crate) max_summary_iterations: usize,
    /// Type-erased call handler installed by [`analyze`](Self::analyze) so that
    /// [`interpret_block`] can dispatch nested calls through [`EvalCall`]
    /// without requiring `L: EvalCall` in its own bounds.
    pub(crate) call_handler: Option<
        fn(
            &mut AbstractInterpreter<'ir, V, S, E, G>,
            SpecializedFunction,
            CompileStage,
            &[V],
        ) -> Result<AnalysisResult<V>, E>,
    >,
    pub(crate) _error: PhantomData<E>,
}

/// Builder for inserting function summaries into an [`AbstractInterpreter`].
///
/// Obtained via [`AbstractInterpreter::insert_summary`] or
/// [`AbstractInterpreter::insert_summary_in_stage`].
pub struct SummaryInserter<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    interp: &'a mut AbstractInterpreter<'ir, V, S, E, G>,
    key: SummaryKey,
}

impl<V: Clone, S: StageMeta, E, G> SummaryInserter<'_, '_, V, S, E, G> {
    /// Insert an immutable summary. Analysis will never re-analyze this function.
    pub fn fixed(self, result: AnalysisResult<V>) {
        self.interp
            .summaries
            .entry(self.key)
            .or_default()
            .set_fixed(result);
    }

    /// Insert a refinable seed. Analysis may improve upon this summary.
    pub fn seed(self, args: Vec<V>, result: AnalysisResult<V>) {
        self.interp
            .summaries
            .entry(self.key)
            .or_default()
            .push_entry(args, result);
    }
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> AbstractInterpreter<'ir, V, S, E, ()>
where
    S: StageMeta,
{
    /// Create an abstract interpreter with unit global state.
    ///
    /// The interpreter is rooted at `stage` when no frame is active.
    pub fn new(pipeline: &'ir Pipeline<S>, stage: CompileStage) -> Self {
        Self::new_with_global(pipeline, stage, ())
    }
}

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: StageMeta,
{
    /// Create an abstract interpreter with explicit global state.
    ///
    /// The interpreter is rooted at `stage` when no frame is active.
    pub fn new_with_global(pipeline: &'ir Pipeline<S>, stage: CompileStage, global: G) -> Self {
        Self {
            pipeline,
            root_stage: stage,
            global,
            widening_strategy: WideningStrategy::AllJoins,
            max_iterations: 1000,
            narrowing_iterations: 3,
            frames: Vec::new(),
            summaries: FxHashMap::default(),
            max_depth: None,
            max_summary_iterations: 100,
            call_handler: None,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: StageMeta,
{
    /// Configure widening behavior used at fixpoint join points.
    pub fn with_widening(mut self, strategy: WideningStrategy) -> Self {
        self.widening_strategy = strategy;
        self
    }

    /// Configure the maximum worklist iterations in one `run_forward` pass.
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Configure post-fixpoint narrowing iterations.
    pub fn with_narrowing_iterations(mut self, n: usize) -> Self {
        self.narrowing_iterations = n;
        self
    }

    /// Configure maximum frame depth for recursive analysis.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Configure maximum outer summary refinement iterations per function.
    pub fn with_max_summary_iterations(mut self, n: usize) -> Self {
        self.max_summary_iterations = n;
        self
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
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

    pub(crate) fn summary_key(stage: CompileStage, callee: SpecializedFunction) -> SummaryKey {
        SummaryKey { stage, callee }
    }

    fn current_summary_stage(&self) -> CompileStage {
        self.frames
            .last()
            .map(Frame::stage)
            .unwrap_or(self.root_stage)
    }

    /// Look up the best cached summary for `callee` in the interpreter's
    /// current active stage.
    pub fn summary(&self, callee: SpecializedFunction, args: &[V]) -> Option<&AnalysisResult<V>>
    where
        V: AbstractValue + Clone,
    {
        self.summary_in_stage(self.current_summary_stage(), callee, args)
    }

    /// Look up the best cached summary for `(stage, callee)` given `args`.
    pub fn summary_in_stage(
        &self,
        stage: CompileStage,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Option<&AnalysisResult<V>>
    where
        V: AbstractValue + Clone,
    {
        self.summaries
            .get(&Self::summary_key(stage, callee))?
            .lookup(args)
    }

    /// Look up the full summary cache for `callee` in the active stage.
    pub fn summary_cache(&self, callee: SpecializedFunction) -> Option<&SummaryCache<V>> {
        self.summary_cache_in_stage(self.current_summary_stage(), callee)
    }

    /// Look up the full summary cache for `(stage, callee)`.
    pub fn summary_cache_in_stage(
        &self,
        stage: CompileStage,
        callee: SpecializedFunction,
    ) -> Option<&SummaryCache<V>> {
        self.summaries.get(&Self::summary_key(stage, callee))
    }

    /// Return a builder for inserting a function summary in the active stage.
    pub fn insert_summary(
        &mut self,
        callee: SpecializedFunction,
    ) -> SummaryInserter<'_, 'ir, V, S, E, G> {
        self.insert_summary_in_stage(self.current_summary_stage(), callee)
    }

    /// Return a builder for inserting a function summary in `stage`.
    pub fn insert_summary_in_stage(
        &mut self,
        stage: CompileStage,
        callee: SpecializedFunction,
    ) -> SummaryInserter<'_, 'ir, V, S, E, G> {
        SummaryInserter {
            interp: self,
            key: Self::summary_key(stage, callee),
        }
    }

    /// Mark all computed entries for `callee` in the active stage as invalidated.
    pub fn invalidate_summary(&mut self, callee: SpecializedFunction) -> usize {
        self.invalidate_summary_in_stage(self.current_summary_stage(), callee)
    }

    /// Mark all computed entries for `(stage, callee)` as invalidated.
    pub fn invalidate_summary_in_stage(
        &mut self,
        stage: CompileStage,
        callee: SpecializedFunction,
    ) -> usize {
        let Some(cache) = self.summaries.get_mut(&Self::summary_key(stage, callee)) else {
            return 0;
        };
        cache.invalidate()
    }

    /// Remove invalidated entries across all functions, freeing memory.
    pub fn gc_summaries(&mut self) {
        for cache in self.summaries.values_mut() {
            cache.gc();
        }
        self.summaries.retain(|_, cache| !cache.is_empty());
    }

    /// Unconditionally remove all summaries (including user-fixed) for
    /// `callee` in the active stage.
    pub fn remove_summary(&mut self, callee: SpecializedFunction) -> bool {
        self.remove_summary_in_stage(self.current_summary_stage(), callee)
    }

    /// Unconditionally remove all summaries (including user-fixed) for
    /// `(stage, callee)`.
    pub fn remove_summary_in_stage(
        &mut self,
        stage: CompileStage,
        callee: SpecializedFunction,
    ) -> bool {
        self.summaries
            .remove(&Self::summary_key(stage, callee))
            .is_some()
    }
}

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    pub(crate) fn current_frame_ref(&self) -> Result<&Frame<V, FixpointState>, E> {
        self.frames
            .last()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub(crate) fn current_frame_mut_ref(&mut self) -> Result<&mut Frame<V, FixpointState>, E> {
        self.frames
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    pub(crate) fn push_frame(&mut self, frame: Frame<V, FixpointState>) -> Result<(), E> {
        if let Some(max) = self.max_depth {
            if self.frames.len() >= max {
                return Err(InterpreterError::MaxDepthExceeded.into());
            }
        }
        self.frames.push(frame);
        Ok(())
    }

    pub(crate) fn pop_frame(&mut self) -> Result<Frame<V, FixpointState>, E> {
        self.frames
            .pop()
            .ok_or_else(|| InterpreterError::NoFrame.into())
    }

    fn active_stage_from_frames(&self) -> CompileStage {
        self.frames
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
}

// -- Interpreter trait impl -------------------------------------------------

impl<'ir, V, S, E, G> Interpreter<'ir> for AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Value = V;
    type Error = E;
    type Ext = std::convert::Infallible;
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
