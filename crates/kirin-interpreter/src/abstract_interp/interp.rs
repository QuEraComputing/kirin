use std::marker::PhantomData;

use kirin_ir::{
    CompileStage, CompileStageInfo, Pipeline, ResultValue, SSAValue, SpecializedFunction,
};
use rustc_hash::FxHashMap;

use super::{FixpointState, SummaryCache};
use crate::result::AnalysisResult;
use crate::widening::WideningStrategy;
use crate::{AbstractValue, Frame, Interpreter, InterpreterError};

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
    S: CompileStageInfo,
{
    pub(crate) pipeline: &'ir Pipeline<S>,
    pub(crate) active_stage: CompileStage,
    pub(crate) global: G,
    pub(crate) frames: Vec<Frame<V, FixpointState>>,
    pub(crate) widening_strategy: WideningStrategy,
    pub(crate) max_iterations: usize,
    pub(crate) narrowing_iterations: usize,
    pub(crate) summaries: FxHashMap<SpecializedFunction, SummaryCache<V>>,
    pub(crate) max_depth: Option<usize>,
    pub(crate) max_summary_iterations: usize,
    /// Type-erased call handler installed by [`analyze`](Self::analyze) so that
    /// [`interpret_block`] can dispatch nested calls through [`CallSemantics`]
    /// without requiring `L: CallSemantics` in its own bounds.
    pub(crate) call_handler: Option<
        fn(
            &mut AbstractInterpreter<'ir, V, S, E, G>,
            SpecializedFunction,
            &[V],
        ) -> Result<AnalysisResult<V>, E>,
    >,
    pub(crate) _error: PhantomData<E>,
}

/// Builder for inserting function summaries into an [`AbstractInterpreter`].
///
/// Obtained via [`AbstractInterpreter::insert_summary`].
pub struct SummaryInserter<'a, 'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    interp: &'a mut AbstractInterpreter<'ir, V, S, E, G>,
    callee: SpecializedFunction,
}

impl<V: Clone, S: CompileStageInfo, E, G> SummaryInserter<'_, '_, V, S, E, G> {
    /// Insert an immutable summary. Analysis will never re-analyze this function.
    pub fn fixed(self, result: AnalysisResult<V>) {
        self.interp
            .summaries
            .entry(self.callee)
            .or_default()
            .set_fixed(result);
    }

    /// Insert a refinable seed. Analysis may improve upon this summary.
    pub fn seed(self, args: Vec<V>, result: AnalysisResult<V>) {
        self.interp
            .summaries
            .entry(self.callee)
            .or_default()
            .push_entry(args, result);
    }
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> AbstractInterpreter<'ir, V, S, E, ()>
where
    S: CompileStageInfo,
{
    pub fn new(pipeline: &'ir Pipeline<S>, active_stage: CompileStage) -> Self {
        Self {
            pipeline,
            active_stage,
            global: (),
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

    /// Attach global state, transforming `G` from `()` to the provided type.
    pub fn with_global<G>(self, global: G) -> AbstractInterpreter<'ir, V, S, E, G> {
        AbstractInterpreter {
            pipeline: self.pipeline,
            active_stage: self.active_stage,
            global,
            widening_strategy: self.widening_strategy,
            max_iterations: self.max_iterations,
            narrowing_iterations: self.narrowing_iterations,
            frames: self.frames,
            summaries: self.summaries,
            max_depth: self.max_depth,
            max_summary_iterations: self.max_summary_iterations,
            call_handler: None,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn with_widening(mut self, strategy: WideningStrategy) -> Self {
        self.widening_strategy = strategy;
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_narrowing_iterations(mut self, n: usize) -> Self {
        self.narrowing_iterations = n;
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_max_summary_iterations(mut self, n: usize) -> Self {
        self.max_summary_iterations = n;
        self
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn global(&self) -> &G {
        &self.global
    }

    pub fn global_mut(&mut self) -> &mut G {
        &mut self.global
    }

    /// Look up the best cached summary for `callee` given `args`.
    ///
    /// Returns the fixed summary if one exists, otherwise finds the tightest
    /// non-invalidated entry whose cached args subsume the query.
    pub fn summary(&self, callee: SpecializedFunction, args: &[V]) -> Option<&AnalysisResult<V>>
    where
        V: AbstractValue + Clone,
    {
        self.summaries.get(&callee)?.lookup(args)
    }

    /// Look up the full summary cache for `callee`.
    pub fn summary_cache(&self, callee: SpecializedFunction) -> Option<&SummaryCache<V>> {
        self.summaries.get(&callee)
    }

    /// Return a builder for inserting a function summary.
    pub fn insert_summary(
        &mut self,
        callee: SpecializedFunction,
    ) -> SummaryInserter<'_, 'ir, V, S, E, G> {
        SummaryInserter {
            interp: self,
            callee,
        }
    }

    /// Mark all computed entries for `callee` as invalidated so the next
    /// [`analyze`](Self::analyze) call re-runs the analysis. Invalidated
    /// entries are retained (for inspection) until
    /// [`gc_summaries`](Self::gc_summaries) is called.
    ///
    /// User-fixed summaries are **not** affected.
    ///
    /// Returns the number of entries invalidated.
    pub fn invalidate_summary(&mut self, callee: SpecializedFunction) -> usize {
        let Some(cache) = self.summaries.get_mut(&callee) else {
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

    /// Unconditionally remove all summaries (including user-fixed) for `callee`.
    ///
    /// Returns `true` if a cache entry was present.
    pub fn remove_summary(&mut self, callee: SpecializedFunction) -> bool {
        self.summaries.remove(&callee).is_some()
    }
}

// -- Interpreter trait impl -------------------------------------------------

impl<'ir, V, S, E, G> Interpreter for AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    type Value = V;
    type Error = E;
    type Ext = std::convert::Infallible;
    type StageInfo = S;

    fn read_ref(&self, value: SSAValue) -> Result<&V, E> {
        self.frames
            .last()
            .and_then(|f| f.read(value))
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.frames
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())?
            .write(result, value);
        Ok(())
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.active_stage
    }
}
