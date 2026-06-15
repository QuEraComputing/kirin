use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, HasBottom, Pipeline, Product, SSAValue, SpecializedFunction, StageMeta,
    Statement, Widen,
};

use crate::ctx::EngineEnv;
use crate::{
    CallEffect, Callee, Effect, Env, EnvIndex, EnvStackStore, Interp, InterpDispatch,
    InterpreterError, Linker, SameStageLinker, Scope, ScopeBody, ScopeStep, StageQuery, query,
};

// ===========================================================================
// Pluggable policy seams
//
// The abstract engine owns the interprocedural worklist, but the two
// decisions a custom analysis needs to control are factored out so they are
// not hard-coded in the engine:
//
//   * `CallContext` — how function summaries are *keyed* (context sensitivity).
//   * `AbstractControl` — how abstract states are *combined* at merge points
//     (the join/widen operator that drives explore/join and termination).
//
// One policy object implements both; the default is context-insensitive with
// join-then-widen, reproducing the engine's prior behavior exactly.
// ===========================================================================

/// Summary-key policy: maps a resolved call target plus its abstract arguments
/// to the key under which that function's entry/return summary is tracked.
///
/// The default ([`DefaultPolicy`]) is context-*insensitive*
/// (`Key = (stage, specialization)`), so all call sites of a function share one
/// summary. A context-*sensitive* policy returns distinct keys for distinct
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

/// Explore/join policy: combines an `incoming` abstract state into the
/// `current` state at a merge point (block entry, loop head, function entry),
/// deciding join vs. widening from the number of prior merges (`visits`).
///
/// Factored out of the engine so the lattice-combination + widening strategy is
/// swappable and not hard-coded into the traversal.
pub trait AbstractControl<V> {
    fn merge(
        &self,
        current: &Product<V>,
        incoming: &Product<V>,
        visits: usize,
    ) -> Result<Product<V>, InterpreterError>;
}

/// Default abstract policy: context-insensitive keys and join-until-`widen_after`
/// then widen. Reproduces the engine's prior behavior.
#[derive(Clone, Copy, Debug)]
pub struct DefaultPolicy {
    pub widen_after: usize,
}

impl Default for DefaultPolicy {
    fn default() -> Self {
        Self { widen_after: 3 }
    }
}

impl<V> CallContext<V> for DefaultPolicy {
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

impl<V> AbstractControl<V> for DefaultPolicy
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
/// domain `V: Widen + Lattice`. The engine owns every fixpoint (CFG block
/// worklists, hook-driven scope loops, interprocedural summaries); the two
/// customizable decisions — summary keying and join/widen — are the policy `P`
/// ([`CallContext`] + [`AbstractControl`]), defaulting to [`DefaultPolicy`].
///
/// ```ignore
/// let mut analysis = AbstractInterpreter::<Stage, ConstPropValue, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let result = analysis.analyze_by_name("source", "abs", [ConstPropValue::Const(7)])?;
/// ```
pub struct AbstractInterpreter<'ir, S: StageMeta, V, E, Lk = SameStageLinker, P = DefaultPolicy>
where
    P: CallContext<V>,
{
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    summaries: HashMap<<P as CallContext<V>>::Key, FnInfo<V, <P as CallContext<V>>::Key>>,
    worklist: VecDeque<<P as CallContext<V>>::Key>,
    queued: HashSet<<P as CallContext<V>>::Key>,
    policy: P,
    max_iterations: usize,
    current: Option<<P as CallContext<V>>::Key>,
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

impl<'ir, S: StageMeta, V, E, P> AbstractInterpreter<'ir, S, V, E, SameStageLinker, P>
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
            policy: P::default(),
            max_iterations: 1000,
            current: None,
            _marker: PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk, P> AbstractInterpreter<'ir, S, V, E, Lk, P>
where
    P: CallContext<V>,
{
    /// Swap the calling-convention component (the [`Linker`]).
    pub fn with_linker<Lk2>(self, linker: Lk2) -> AbstractInterpreter<'ir, S, V, E, Lk2, P> {
        AbstractInterpreter {
            pipeline: self.pipeline,
            linker,
            store: self.store,
            summaries: self.summaries,
            worklist: self.worklist,
            queued: self.queued,
            policy: self.policy,
            max_iterations: self.max_iterations,
            current: self.current,
            _marker: PhantomData,
        }
    }

    /// Swap the summary-key / join-widen policy. Changes the [`CallContext::Key`]
    /// type, so this resets the (empty) summary tables.
    pub fn with_policy<P2>(self, policy: P2) -> AbstractInterpreter<'ir, S, V, E, Lk, P2>
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
            policy,
            max_iterations: self.max_iterations,
            current: None,
            _marker: PhantomData,
        }
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S: StageMeta, V, E, Lk> AbstractInterpreter<'ir, S, V, E, Lk, DefaultPolicy> {
    /// Number of joins at a loop head or function entry before switching from
    /// join to widening (only available with the [`DefaultPolicy`]).
    pub fn widen_after(mut self, joins: usize) -> Self {
        self.policy.widen_after = joins;
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

impl<'ir, S, V, E, Lk, P> Interp for AbstractInterpreter<'ir, S, V, E, Lk, P>
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

/// Outcome of evaluating a structured scope body once.
enum BodyOutcome<V> {
    Yielded(Product<V>),
    /// Every path through the body left via `Return`; execution after the
    /// scope is unreachable on this path.
    Returned,
}

impl<'ir, S, V, E, Lk, P> AbstractInterpreter<'ir, S, V, E, Lk, P>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Widen,
    E: From<InterpreterError>,
    Lk: Linker<S>,
    P: CallContext<V> + AbstractControl<V>,
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
        let key = self.policy.key(target.stage, target.function, &args);
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
                *current = self.policy.merge(current, &incoming, 0).map_err(E::from)?
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
                    .policy
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
        let pipeline = self.pipeline;
        let stage_info = pipeline
            .stage(stage)
            .ok_or_else(|| E::from(InterpreterError::MissingStage(stage)))?;
        let scope = stage_info.dispatch_function_entry(stage, body, entry, env, self)?;

        let mut ret_acc: Option<Product<V>> = None;
        let result = self.eval_scope_root(stage, env, scope, &mut ret_acc);
        self.store.free(env).map_err(E::from)?;
        self.current = previous;
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
                Some(old) => self.policy.merge(&old, &new_ret, 0).map_err(E::from)?,
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

    /// Evaluate a function-entry scope (region or single block).
    fn eval_scope_root(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        scope: Scope<V, E>,
        ret_acc: &mut Option<Product<V>>,
    ) -> Result<(), E> {
        match scope.body() {
            ScopeBody::Region(region) => {
                let entry = query::region_entry(self.pipeline, stage, region)
                    .map_err(E::from)?
                    .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?;
                self.eval_cfg(stage, env, entry, scope_args(scope), ret_acc)
            }
            ScopeBody::Block(block) => self.eval_cfg(stage, env, block, scope_args(scope), ret_acc),
            ScopeBody::Immediate => {
                self.join_into(ret_acc, scope_args(scope))?;
                Ok(())
            }
        }
    }

    /// Worklist evaluation of a CFG rooted at `entry`. Block parameters join
    /// across incoming edges and widen after `widen_after` visits.
    fn eval_cfg(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        entry: Block,
        args: Product<V>,
        ret_acc: &mut Option<Product<V>>,
    ) -> Result<(), E> {
        let mut block_in: HashMap<Block, Product<V>> = HashMap::new();
        let mut visits: HashMap<Block, usize> = HashMap::new();
        let mut pending: VecDeque<Block> = VecDeque::new();
        let mut queued: HashSet<Block> = HashSet::new();

        block_in.insert(entry, args);
        pending.push_back(entry);
        queued.insert(entry);

        let mut iterations = 0usize;
        while let Some(block) = pending.pop_front() {
            queued.remove(&block);
            iterations += 1;
            if iterations > self.max_iterations {
                return Err(E::from(InterpreterError::FixpointDiverged));
            }

            let in_args = block_in
                .get(&block)
                .cloned()
                .ok_or_else(|| E::from(InterpreterError::Custom("missing block entry state")))?;
            self.bind_block_args(stage, env, block, &in_args)?;

            let mut cursor =
                query::first_statement(self.pipeline, stage, block).map_err(E::from)?;
            while let Some(statement) = cursor {
                cursor = query::next_statement(self.pipeline, stage, block, statement)
                    .map_err(E::from)?;
                match self.dispatch(stage, statement, env)? {
                    Effect::Next => {}
                    Effect::Jump(edge) => {
                        self.flow(
                            &mut block_in,
                            &mut visits,
                            &mut pending,
                            &mut queued,
                            edge.target,
                            edge.args,
                        )?;
                        break;
                    }
                    Effect::Branch(edges) => {
                        for edge in edges {
                            self.flow(
                                &mut block_in,
                                &mut visits,
                                &mut pending,
                                &mut queued,
                                edge.target,
                                edge.args,
                            )?;
                        }
                        break;
                    }
                    Effect::Return(values) => {
                        self.join_into(ret_acc, values)?;
                        break;
                    }
                    Effect::Yield(_) => {
                        return Err(E::from(InterpreterError::UnexpectedYield(statement)));
                    }
                    Effect::Call(call) => self.eval_call(stage, env, call)?,
                    Effect::Enter(scope) => {
                        let results = scope_result_slots(&scope);
                        match self.eval_scope(stage, env, scope, ret_acc)? {
                            Some(values) => self.write_results(env, &results, values)?,
                            None => break,
                        }
                    }
                    Effect::EnterAny(scopes) => {
                        match self.eval_scope_alternatives(stage, env, scopes, ret_acc)? {
                            Some(()) => {}
                            None => break,
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Join edge arguments into a successor's entry state via the policy,
    /// enqueueing it when the state changes.
    fn flow(
        &self,
        block_in: &mut HashMap<Block, Product<V>>,
        visits: &mut HashMap<Block, usize>,
        pending: &mut VecDeque<Block>,
        queued: &mut HashSet<Block>,
        target: Block,
        args: Product<V>,
    ) -> Result<(), E> {
        let changed = match block_in.get_mut(&target) {
            None => {
                block_in.insert(target, args);
                true
            }
            Some(old) => {
                let count = visits.entry(target).or_insert(0);
                *count += 1;
                let joined = self.policy.merge(old, &args, *count).map_err(E::from)?;
                if joined != *old {
                    *old = joined;
                    true
                } else {
                    false
                }
            }
        };
        if changed && queued.insert(target) {
            pending.push_back(target);
        }
        Ok(())
    }

    /// Evaluate a hook-driven structured scope to its local fixpoint. Returns
    /// the joined finish results, or `None` when no path finishes.
    fn eval_scope(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        scope: Scope<V, E>,
        ret_acc: &mut Option<Product<V>>,
    ) -> Result<Option<Product<V>>, E> {
        let block = match scope.body() {
            ScopeBody::Immediate => return Ok(Some(scope_args(scope))),
            ScopeBody::Block(block) => block,
            ScopeBody::Region(_) => {
                return Err(E::from(InterpreterError::Custom(
                    "inline region scopes are not supported by the abstract interpreter",
                )));
            }
        };

        let (mut entry, mut hook) = scope_args_hook(scope);
        let mut finish: Option<Product<V>> = None;
        let mut iterations = 0usize;
        loop {
            iterations += 1;
            if iterations > self.max_iterations {
                return Err(E::from(InterpreterError::FixpointDiverged));
            }
            self.bind_block_args(stage, env, block, &entry)?;
            let outcome = self.eval_scope_body(stage, env, block, ret_acc)?;
            let BodyOutcome::Yielded(yielded) = outcome else {
                break;
            };
            match hook.take() {
                None => {
                    self.join_into(&mut finish, yielded)?;
                    break;
                }
                Some(h) => {
                    let step = h.on_yield(&entry, yielded, &mut EngineEnv { interp: self, env })?;
                    let (args, next_hook) = match step {
                        ScopeStep::Finish(results) => {
                            self.join_into(&mut finish, results)?;
                            break;
                        }
                        ScopeStep::Repeat { args, hook } => (args, hook),
                        ScopeStep::RepeatOrFinish {
                            args,
                            results,
                            hook,
                        } => {
                            self.join_into(&mut finish, results)?;
                            (args, hook)
                        }
                    };
                    let joined = self
                        .policy
                        .merge(&entry, &args, iterations)
                        .map_err(E::from)?;
                    if joined == entry {
                        // Stable entry state: re-running the body adds nothing.
                        break;
                    }
                    entry = joined;
                    hook = Some(next_hook);
                }
            }
        }
        Ok(finish)
    }

    /// Evaluate undecided scope alternatives, joining results into the shared
    /// result slots. Returns `None` when no alternative finishes.
    fn eval_scope_alternatives(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        scopes: Vec<Scope<V, E>>,
        ret_acc: &mut Option<Product<V>>,
    ) -> Result<Option<()>, E> {
        let results_slots = scopes.first().map(scope_result_slots).unwrap_or_default();
        let mut acc: Option<Product<V>> = None;
        for scope in scopes {
            if let Some(values) = self.eval_scope(stage, env, scope, ret_acc)? {
                self.join_into(&mut acc, values)?;
            }
        }
        match acc {
            Some(values) => {
                self.write_results(env, &results_slots, values)?;
                Ok(Some(()))
            }
            None => Ok(None),
        }
    }

    /// Linear evaluation of a structured scope body (a single block whose
    /// terminator yields or returns).
    fn eval_scope_body(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        block: Block,
        ret_acc: &mut Option<Product<V>>,
    ) -> Result<BodyOutcome<V>, E> {
        let mut cursor = query::first_statement(self.pipeline, stage, block).map_err(E::from)?;
        while let Some(statement) = cursor {
            cursor =
                query::next_statement(self.pipeline, stage, block, statement).map_err(E::from)?;
            match self.dispatch(stage, statement, env)? {
                Effect::Next => {}
                Effect::Yield(values) => return Ok(BodyOutcome::Yielded(values)),
                Effect::Return(values) => {
                    self.join_into(ret_acc, values)?;
                    return Ok(BodyOutcome::Returned);
                }
                Effect::Call(call) => self.eval_call(stage, env, call)?,
                Effect::Enter(scope) => {
                    let results = scope_result_slots(&scope);
                    match self.eval_scope(stage, env, scope, ret_acc)? {
                        Some(values) => self.write_results(env, &results, values)?,
                        None => return Ok(BodyOutcome::Returned),
                    }
                }
                Effect::EnterAny(scopes) => {
                    match self.eval_scope_alternatives(stage, env, scopes, ret_acc)? {
                        Some(()) => {}
                        None => return Ok(BodyOutcome::Returned),
                    }
                }
                Effect::Jump(_) | Effect::Branch(_) => {
                    return Err(E::from(InterpreterError::Custom(
                        "CFG transfer inside a structured scope body",
                    )));
                }
            }
        }
        Err(E::from(InterpreterError::BlockFellThrough(block)))
    }

    /// Summarize a call: key it through the policy, join arguments into the
    /// callee's entry summary, and read its current return summary (bottom
    /// until the callee converges).
    fn eval_call(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        call: CallEffect<V>,
    ) -> Result<(), E> {
        let resolve_stage = call.stage.unwrap_or(stage);
        let target = self
            .linker
            .resolve(self.pipeline, resolve_stage, &call.callee)
            .map_err(E::from)?;
        let key = self.policy.key(target.stage, target.function, &call.args);
        self.join_entry(key.clone(), target.stage, target.body, call.args)?;
        // Register the caller — including same-key (self-)recursion. When a
        // recursive summary's return value rises, its own call site must be
        // re-analyzed to observe it; with a `caller != key` guard the recursion
        // only ever sees the base case (e.g. factorial collapses to `Const(1)`
        // instead of sound `Top`).
        if let Some(caller) = self.current.clone()
            && let Some(info) = self.summaries.get_mut(&key)
        {
            info.callers.insert(caller);
        }
        let ret = self.summaries.get(&key).and_then(|info| info.ret.clone());
        match ret {
            Some(values) => self.write_results(env, &call.results, values),
            None => {
                // Callee has not converged yet: results are unreached.
                for slot in call.results.iter().copied() {
                    self.env_write(env, slot, V::bottom())?;
                }
                Ok(())
            }
        }
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

// Small accessors that move/borrow `Scope`'s parts without exposing its
// pub(crate) fields outside this crate.
type ScopeEntry<V, E> = (Product<V>, Option<Box<dyn crate::ScopeHook<V, E>>>);

fn scope_args<V, E>(scope: Scope<V, E>) -> Product<V> {
    let Scope { args, .. } = scope;
    args
}

fn scope_args_hook<V, E>(scope: Scope<V, E>) -> ScopeEntry<V, E> {
    let Scope { args, hook, .. } = scope;
    (args, hook)
}

fn scope_result_slots<V, E>(scope: &Scope<V, E>) -> Product<SSAValue> {
    scope.results.clone()
}
