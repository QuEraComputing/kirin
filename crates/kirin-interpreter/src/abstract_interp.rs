use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, HasBottom, Pipeline, Product, Region, SSAValue, SpecializedFunction,
    StageMeta, Statement, Widen,
};

use crate::{
    AbstractCompletion, AbstractFrameBuild, AbstractFrameDriver, AbstractFunctionFrame, CallEffect,
    Callee, Effect, Env, EnvIndex, EnvStackStore, Frame, FrameDriver, FrameEffect, FunctionTarget,
    Interp, InterpDispatch, InterpreterError, Linker, SameStageLinker, Scope, StageQuery,
    StandardAbstractFrame, query,
};

// ===========================================================================
// Pluggable analysis seams (policy `P`)
//
// The two decisions a custom analysis controls — distinct from *traversal*,
// which is the frame type `F`:
//
//   * `CallContext` — how function summaries are *keyed* (context sensitivity).
//   * `WideningStrategy` — how abstract states are *combined* at merge points
//     (the join/widen operator that drives explore/join and termination).
//
// One analysis object implements both; the default is context-insensitive with
// join-then-widen, reproducing the engine's prior behavior exactly.
// ===========================================================================

/// Summary-key strategy: maps a resolved call target plus its abstract arguments
/// to the key under which that function's entry/return summary is tracked.
///
/// The default ([`ContextInsensitive`]) is context-*insensitive*
/// (`Key = (stage, specialization)`), so all call sites of a function share one
/// summary. A context-*sensitive* strategy returns distinct keys for distinct
/// argument abstractions (see the bounded-arg-tuple policy used by constprop),
/// which is what makes precise recursive analysis possible.
pub trait CallContext<V> {
    type Key: Clone + Eq + Hash;

    fn key(
        &mut self,
        stage: CompileStage,
        function: SpecializedFunction,
        args: &Product<V>,
    ) -> Self::Key;
}

/// Explore/join strategy: combines an `incoming` abstract state into the
/// `current` state at a merge point (block entry, loop head, function entry),
/// deciding join vs. widening from the number of prior merges (`visits`).
///
/// Factored out of the engine so the lattice-combination + widening strategy is
/// swappable and not hard-coded into the traversal.
pub trait WideningStrategy<V> {
    fn merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, InterpreterError>;
}

/// Default analysis: context-insensitive keys and join-until-`widen_after`
/// then widen. Reproduces the engine's prior behavior.
#[derive(Clone, Copy, Debug)]
pub struct ContextInsensitive {
    pub widen_after: usize,
}

impl Default for ContextInsensitive {
    fn default() -> Self {
        Self { widen_after: 3 }
    }
}

impl<V> CallContext<V> for ContextInsensitive {
    type Key = (CompileStage, SpecializedFunction);

    fn key(
        &mut self,
        stage: CompileStage,
        function: SpecializedFunction,
        _args: &Product<V>,
    ) -> Self::Key {
        (stage, function)
    }
}

impl<V> WideningStrategy<V> for ContextInsensitive
where
    V: Clone + Widen,
{
    fn merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, InterpreterError> {
        join_products(current, incoming, visits > self.widen_after)
    }
}

/// Lattice-based abstract interpreter with interprocedural fixpoint solving.
///
/// Runs the same dialect [`Interpretable`](crate::Interpretable) rules as
/// [`ConcreteInterpreter`](crate::ConcreteInterpreter), over a lattice value
/// domain `V: Widen + Lattice`. Like the concrete engine, traversal lives in
/// frames: the total abstract frame type `F` (default
/// [`StandardAbstractFrame`]) owns the CFG block worklist, branch exploration,
/// scope fixpoints, and call summarization, and the engine just runs the frame
/// stack (`run_frames`). A custom `F` (its own enum reusing the standard frames
/// via [`AbstractFrameBuild`]) customizes/observes abstract traversal without
/// forking the engine. The orthogonal *analysis policy* `P` ([`CallContext`] +
/// [`WideningStrategy`], default [`ContextInsensitive`]) controls summary keying
/// and join/widen; the interprocedural protocol stays atomic in the engine.
///
/// ```ignore
/// let mut analysis = AbstractInterpreter::<Stage, ConstPropValue, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let result = analysis.analyze_by_name("source", "abs", [ConstPropValue::Const(7)])?;
/// ```
pub struct AbstractInterpreter<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    F = StandardAbstractFrame<V, E, <P as CallContext<V>>::Key>,
> where
    P: CallContext<V>,
{
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    summaries: HashMap<<P as CallContext<V>>::Key, FnInfo<V, <P as CallContext<V>>::Key>>,
    worklist: VecDeque<<P as CallContext<V>>::Key>,
    queued: HashSet<<P as CallContext<V>>::Key>,
    analysis: P,
    max_iterations: usize,
    current: Option<<P as CallContext<V>>::Key>,
    /// The per-function frame stack driven by `run_frames` (empty between
    /// interprocedural worklist items).
    frames: Vec<F>,
    /// Return accumulator for the function currently being evaluated. Reset at
    /// the start of every `eval_function`, so no state leaks between functions.
    ret_acc: Option<Product<V>>,
    _marker: PhantomData<fn() -> E>,
}

struct FnInfo<V, K> {
    stage: CompileStage,
    body: Statement,
    entry: Product<V>,
    entry_joins: usize,
    ret: Option<Product<V>>,
    callers: HashSet<K>,
}

impl<'ir, S: StageMeta, V, E, P, F> AbstractInterpreter<'ir, S, V, E, SameStageLinker, P, F>
where
    P: CallContext<V> + Default,
{
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            linker: SameStageLinker,
            store: EnvStackStore::new(),
            summaries: HashMap::new(),
            worklist: VecDeque::new(),
            queued: HashSet::new(),
            analysis: P::default(),
            max_iterations: 1000,
            current: None,
            frames: Vec::new(),
            ret_acc: None,
            _marker: PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk, P, F> AbstractInterpreter<'ir, S, V, E, Lk, P, F>
where
    P: CallContext<V>,
{
    /// Swap the calling-convention component (the [`Linker`]). Preserves the
    /// frame type `F`.
    pub fn with_linker<Lk2>(self, linker: Lk2) -> AbstractInterpreter<'ir, S, V, E, Lk2, P, F> {
        AbstractInterpreter {
            pipeline: self.pipeline,
            linker,
            store: self.store,
            summaries: self.summaries,
            worklist: self.worklist,
            queued: self.queued,
            analysis: self.analysis,
            max_iterations: self.max_iterations,
            current: self.current,
            frames: self.frames,
            ret_acc: self.ret_acc,
            _marker: PhantomData,
        }
    }

    /// Swap the analysis policy (context abstraction + join/widen). Changes the
    /// [`CallContext::Key`] type, so this resets the (empty) summary tables and
    /// the (default) frame type.
    pub fn with_analysis<P2>(self, analysis: P2) -> AbstractInterpreter<'ir, S, V, E, Lk, P2>
    where
        P2: CallContext<V>,
    {
        AbstractInterpreter {
            pipeline: self.pipeline,
            linker: self.linker,
            store: self.store,
            summaries: HashMap::new(),
            worklist: VecDeque::new(),
            queued: HashSet::new(),
            analysis,
            max_iterations: self.max_iterations,
            current: None,
            frames: Vec::new(),
            ret_acc: None,
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S: StageMeta, V, E, Lk, F> AbstractInterpreter<'ir, S, V, E, Lk, ContextInsensitive, F> {
    /// Number of joins at a loop head or function entry before switching from
    /// join to widening (only available with the [`ContextInsensitive`]).
    pub fn widen_after(mut self, joins: usize) -> Self {
        self.analysis.widen_after = joins;
        self
    }

    /// Inspect the return summary of an analyzed function (context-insensitive
    /// keying only).
    pub fn return_summary(
        &self,
        stage: CompileStage,
        function: SpecializedFunction,
    ) -> Option<&Product<V>> {
        self.summaries
            .get(&(stage, function))
            .and_then(|info| info.ret.as_ref())
    }
}

impl<'ir, S, V, E, Lk, P, F> Interp for AbstractInterpreter<'ir, S, V, E, Lk, P, F>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
{
    type Value = V;
    type Error = E;

    /// Reads of values not yet bound are `bottom` (unreached code).
    fn env_read(&self, env: EnvIndex, value: SSAValue) -> Result<V, E> {
        match self.store.read(env, value) {
            Ok(value) => Ok(value),
            Err(InterpreterError::UnboundValue { .. }) => Ok(V::bottom()),
            Err(error) => Err(E::from(error)),
        }
    }

    fn env_write(&mut self, env: EnvIndex, value: SSAValue, data: V) -> Result<(), E> {
        self.store.write(env, value, data).map_err(E::from)
    }
}

// The IR-query / dispatch capability surface shared with the concrete engine.
// Identical in shape to `ConcreteInterpreter`'s; `resolve_call` routes through
// the same linker component.
impl<'ir, S, V, E, Lk, P, F> FrameDriver for AbstractInterpreter<'ir, S, V, E, Lk, P, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V>,
{
    fn alloc_env(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free_env(&mut self, env: EnvIndex) -> Result<(), E> {
        self.store.free(env).map_err(E::from)
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
        env: EnvIndex,
    ) -> Result<Effect<V, E>, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        info.dispatch_statement(stage, statement, env, self)
    }

    fn enter_function(
        &mut self,
        stage: CompileStage,
        body: Statement,
        args: Product<V>,
        env: EnvIndex,
    ) -> Result<Scope<V, E>, E> {
        let pipeline = self.pipeline;
        let info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        info.dispatch_function_entry(stage, body, args, env, self)
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

// The abstract-only capability surface the abstract frames drive. The
// interprocedural protocol (`summarize_call`) stays atomic here so frame
// authors cannot break the self-recursion / summary invariants.
impl<'ir, S, V, E, Lk, P, F> AbstractFrameDriver for AbstractInterpreter<'ir, S, V, E, Lk, P, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
{
    type SummaryKey = <P as CallContext<V>>::Key;

    fn analysis_merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, E> {
        self.analysis
            .merge(current, incoming, visits)
            .map_err(E::from)
    }

    fn contribute_return(&mut self, values: Product<V>) -> Result<(), E> {
        // take/put-back so `join_into` can borrow `&self.analysis` and a
        // disjoint local accumulator simultaneously.
        let mut acc = self.ret_acc.take();
        let result = self.join_into(&mut acc, values);
        self.ret_acc = acc;
        result
    }

    fn current_function_key(&self) -> Option<<P as CallContext<V>>::Key> {
        self.current.clone()
    }

    /// Summarize a call: key it through the analysis, join arguments into the
    /// callee's entry summary, record the caller (including same-key recursion),
    /// and write the callee's current return summary (bottom until it converges).
    fn summarize_call(
        &mut self,
        stage: CompileStage,
        call: CallEffect<V>,
        env: EnvIndex,
    ) -> Result<(), E> {
        let CallEffect {
            callee,
            stage: call_stage,
            args,
            results,
        } = call;
        let resolve_stage = call_stage.unwrap_or(stage);
        let target = self
            .linker
            .resolve(self.pipeline, resolve_stage, &callee)
            .map_err(E::from)?;
        let key = self.analysis.key(target.stage, target.function, &args);
        self.join_entry(key.clone(), target.stage, target.body, args)?;
        // Register the caller — including same-key (self-)recursion. When a
        // recursive summary's return value rises, its own call site must be
        // re-analyzed to observe it; without registering the self-dependency the
        // recursion only ever sees the base case (e.g. factorial collapses to
        // `Const(1)` instead of sound `Top`).
        if let Some(caller) = self.current.clone()
            && let Some(info) = self.summaries.get_mut(&key)
        {
            info.callers.insert(caller);
        }
        let ret = self.summaries.get(&key).and_then(|info| info.ret.clone());
        match ret {
            Some(values) => self.write_results(env, &results, values),
            None => {
                // Callee has not converged yet: results are unreached.
                for slot in results.iter().copied() {
                    self.env_write(env, slot, V::bottom())?;
                }
                Ok(())
            }
        }
    }

    fn max_iterations(&self) -> usize {
        self.max_iterations
    }
}

// Interprocedural summary bookkeeping, shared by the frame-driver capability
// impl and the analyze loop (no frame-type bound — these are policy, not
// traversal).
impl<'ir, S: StageMeta, V, E, Lk, P, F> AbstractInterpreter<'ir, S, V, E, Lk, P, F>
where
    V: Clone + PartialEq + Widen,
    E: From<InterpreterError>,
    P: CallContext<V> + WideningStrategy<V>,
{
    fn enqueue(&mut self, key: <P as CallContext<V>>::Key) {
        if self.queued.insert(key.clone()) {
            self.worklist.push_back(key);
        }
    }

    /// Join the supplied `incoming` product into an optional accumulator
    /// (used for return/finish products whose arity is unknown until the first
    /// contribution). Never widens.
    fn join_into(&self, acc: &mut Option<Product<V>>, incoming: Product<V>) -> Result<(), E> {
        match acc {
            None => *acc = Some(incoming),
            Some(current) => {
                *current = self
                    .analysis
                    .merge(current, &incoming, 0)
                    .map_err(E::from)?
            }
        }
        Ok(())
    }

    /// Join `args` into the entry summary for `key`, enqueueing on change.
    fn join_entry(
        &mut self,
        key: <P as CallContext<V>>::Key,
        stage: CompileStage,
        body: Statement,
        args: Product<V>,
    ) -> Result<(), E> {
        match self.summaries.get_mut(&key) {
            None => {
                self.summaries.insert(
                    key.clone(),
                    FnInfo {
                        stage,
                        body,
                        entry: args,
                        entry_joins: 0,
                        ret: None,
                        callers: HashSet::new(),
                    },
                );
                self.enqueue(key);
            }
            Some(info) => {
                info.entry_joins += 1;
                let visits = info.entry_joins;
                let joined = self
                    .analysis
                    .merge(&info.entry, &args, visits)
                    .map_err(E::from)?;
                if joined != info.entry {
                    info.entry = joined;
                    self.enqueue(key);
                }
            }
        }
        Ok(())
    }
}

impl<'ir, S, V, E, Lk, P, F> AbstractInterpreter<'ir, S, V, E, Lk, P, F>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Widen + HasBottom,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + WideningStrategy<V>,
    F: Frame<Self, Completion = AbstractCompletion<V>>
        + AbstractFrameBuild<V, E, <P as CallContext<V>>::Key>,
{
    /// Resolve `stage`/`function` by name and analyze. Returns the function's
    /// inferred return product at the fixpoint (empty if it never returns).
    pub fn analyze_by_name(
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
        self.analyze(stage, Callee::Function(function), args)
    }

    /// Run the interprocedural fixpoint from a single entry.
    pub fn analyze(
        &mut self,
        stage: CompileStage,
        callee: Callee,
        args: impl IntoIterator<Item = V>,
    ) -> Result<Product<V>, E> {
        let target = self
            .linker
            .resolve(self.pipeline, stage, &callee)
            .map_err(E::from)?;
        let args: Product<V> = args.into_iter().collect();
        let key = self.analysis.key(target.stage, target.function, &args);
        self.join_entry(key.clone(), target.stage, target.body, args)?;

        let mut iterations = 0usize;
        while let Some(key) = self.worklist.pop_front() {
            self.queued.remove(&key);
            iterations += 1;
            if iterations > self.max_iterations {
                return Err(E::from(InterpreterError::FixpointDiverged));
            }
            self.eval_function(key)?;
        }

        Ok(self
            .summaries
            .get(&key)
            .and_then(|info| info.ret.clone())
            .unwrap_or_default())
    }

    /// Evaluate one function summary by driving its body through the abstract
    /// frame stack, then fold the computed return into the summary and
    /// re-enqueue dependent callers if it changed.
    fn eval_function(&mut self, key: <P as CallContext<V>>::Key) -> Result<(), E> {
        let info = self
            .summaries
            .get(&key)
            .ok_or_else(|| E::from(InterpreterError::Custom("missing function summary")))?;
        let stage = info.stage;
        let body = info.body;
        let entry = info.entry.clone();

        let previous = self.current.replace(key.clone());
        let env = self.store.alloc();
        self.ret_acc = None;
        self.frames
            .push(F::from_function(AbstractFunctionFrame::new(
                stage, body, entry, env,
            )));
        let result = self.run_frames();
        self.store.free(env).map_err(E::from)?;
        self.current = previous;
        let ret_acc = self.ret_acc.take();
        result?;

        // Fold the freshly-computed return product into the summary; re-enqueue
        // callers if it changed.
        let changed_callers: Option<Vec<<P as CallContext<V>>::Key>> = {
            let new_ret = match ret_acc {
                Some(values) => values,
                None => return Ok(()),
            };
            let merged = match self.summaries.get(&key).and_then(|info| info.ret.clone()) {
                None => new_ret,
                Some(old) => self.analysis.merge(&old, &new_ret, 0).map_err(E::from)?,
            };
            let info = self
                .summaries
                .get_mut(&key)
                .ok_or_else(|| E::from(InterpreterError::Custom("missing function summary")))?;
            if info.ret.as_ref() != Some(&merged) {
                info.ret = Some(merged);
                Some(info.callers.iter().cloned().collect())
            } else {
                None
            }
        };
        if let Some(callers) = changed_callers {
            for caller in callers {
                self.enqueue(caller);
            }
        }
        Ok(())
    }

    /// The generic abstract driver loop: pop the top frame, step it, and apply
    /// the resulting [`FrameEffect`]. `Done`/`Complete` bubble synchronously
    /// through parents until one continues or the stack empties. Only
    /// [`AbstractCompletion::FunctionDone`] is valid at the root.
    fn run_frames(&mut self) -> Result<(), E> {
        loop {
            let frame = self
                .frames
                .pop()
                .ok_or_else(|| E::from(InterpreterError::EmptyFrameStack))?;
            let mut effect = frame.step(self)?;
            loop {
                match effect {
                    FrameEffect::Continue(frame) => {
                        self.frames.push(frame);
                        break;
                    }
                    FrameEffect::Push { parent, child } => {
                        self.frames.push(parent);
                        self.frames.push(child);
                        break;
                    }
                    FrameEffect::Done => {
                        let parent = self
                            .frames
                            .pop()
                            .ok_or_else(|| E::from(InterpreterError::EmptyFrameStack))?;
                        effect = parent.resume_done(self)?;
                    }
                    FrameEffect::Complete(completion) => match self.frames.pop() {
                        Some(parent) => {
                            effect = parent.resume(completion, self)?;
                        }
                        None => match completion {
                            AbstractCompletion::FunctionDone => return Ok(()),
                            AbstractCompletion::ScopeFinished(_) => {
                                return Err(E::from(InterpreterError::Custom(
                                    "scope completion reached the frame-stack root",
                                )));
                            }
                        },
                    },
                }
            }
        }
    }
}

/// Element-wise join (or widen) of two products of equal arity.
fn join_products<V>(
    old: &Product<V>,
    new: &Product<V>,
    widen: bool,
) -> Result<Product<V>, InterpreterError>
where
    V: Clone + Widen,
{
    if old.len() != new.len() {
        return Err(InterpreterError::ProductArityMismatch {
            expected: old.len(),
            actual: new.len(),
        });
    }
    Ok(old
        .iter()
        .zip(new.iter())
        .map(|(old, new)| if widen { old.widen(new) } else { old.join(new) })
        .collect())
}
