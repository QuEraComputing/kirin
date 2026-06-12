use std::collections::{HashMap, HashSet, VecDeque};

use kirin_ir::{
    Block, CompileStage, HasBottom, Pipeline, Product, SSAValue, SpecializedFunction, StageMeta,
    Statement, Widen,
};

use crate::ctx::EngineEnv;
use crate::{
    CallEffect, Callee, Effect, Env, EnvIndex, EnvStackStore, Interp, InterpDispatch,
    InterpreterError, Linker, SameStageLinker, Scope, ScopeBody, ScopeStep, StageQuery, query,
};

/// Lattice-based abstract interpreter with interprocedural fixpoint solving.
///
/// Runs the same dialect [`Interpretable`](crate::Interpretable) rules as
/// [`ConcreteInterpreter`](crate::ConcreteInterpreter), over a lattice value
/// domain `V: Widen + Lattice`. The engine owns every fixpoint:
///
/// - **Blocks**: function bodies are evaluated as CFG worklists; block
///   parameters join across incoming edges and widen on repeated visits, so
///   `cf`-style loops converge.
/// - **Scopes**: hook-driven scopes ([`Effect::Enter`]) re-run their body
///   with joined entry arguments until stable, so `scf.for` loops converge
///   without any dialect-side fixpoint code.
/// - **Functions**: call targets are summarized as entry/return products;
///   summary changes re-enqueue dependent functions until convergence.
///
/// Undecided control flow ([`Effect::Branch`], [`Effect::EnterAny`],
/// [`ScopeStep::RepeatOrFinish`]) explores every alternative and joins.
///
/// ```ignore
/// let mut analysis = AbstractInterpreter::<Stage, ConstPropValue, MyError>::new(&pipeline)
///     .with_linker(CrossStageLinker);
/// let result = analysis.analyze_by_name("source", "abs", [ConstPropValue::Const(7)])?;
/// ```
pub struct AbstractInterpreter<'ir, S: StageMeta, V, E, Lk = SameStageLinker> {
    pipeline: &'ir Pipeline<S>,
    linker: Lk,
    store: EnvStackStore<V>,
    summaries: HashMap<FnKey, FnInfo<V>>,
    worklist: VecDeque<FnKey>,
    queued: HashSet<FnKey>,
    widen_after: usize,
    max_iterations: usize,
    current: Option<FnKey>,
    _marker: std::marker::PhantomData<E>,
}

type FnKey = (CompileStage, SpecializedFunction);

struct FnInfo<V> {
    body: Statement,
    entry: Product<V>,
    entry_joins: usize,
    ret: Option<Product<V>>,
    callers: HashSet<FnKey>,
}

impl<'ir, S: StageMeta, V, E> AbstractInterpreter<'ir, S, V, E> {
    pub fn new(pipeline: &'ir Pipeline<S>) -> Self {
        Self {
            pipeline,
            linker: SameStageLinker,
            store: EnvStackStore::new(),
            summaries: HashMap::new(),
            worklist: VecDeque::new(),
            queued: HashSet::new(),
            widen_after: 3,
            max_iterations: 1000,
            current: None,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'ir, S: StageMeta, V, E, Lk> AbstractInterpreter<'ir, S, V, E, Lk> {
    /// Swap the calling-convention component.
    pub fn with_linker<Lk2>(self, linker: Lk2) -> AbstractInterpreter<'ir, S, V, E, Lk2> {
        AbstractInterpreter {
            pipeline: self.pipeline,
            linker,
            store: self.store,
            summaries: self.summaries,
            worklist: self.worklist,
            queued: self.queued,
            widen_after: self.widen_after,
            max_iterations: self.max_iterations,
            current: self.current,
            _marker: std::marker::PhantomData,
        }
    }

    /// Number of joins at a loop head or function entry before switching from
    /// join to widening.
    pub fn widen_after(mut self, joins: usize) -> Self {
        self.widen_after = joins;
        self
    }

    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }
}

impl<'ir, S, V, E, Lk> Interp for AbstractInterpreter<'ir, S, V, E, Lk>
where
    S: StageMeta,
    V: Clone + HasBottom,
    E: From<InterpreterError>,
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

impl<'ir, S, V, E, Lk> AbstractInterpreter<'ir, S, V, E, Lk>
where
    S: StageQuery + InterpDispatch<Self>,
    V: Clone + PartialEq + Widen,
    E: From<InterpreterError>,
    Lk: Linker<S>,
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
        let key = (target.stage, target.function);
        let args: Product<V> = args.into_iter().collect();
        self.join_entry(key, target.body, args)?;

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

    /// Inspect the return summary of an analyzed function.
    pub fn return_summary(
        &self,
        stage: CompileStage,
        function: SpecializedFunction,
    ) -> Option<&Product<V>> {
        self.summaries
            .get(&(stage, function))
            .and_then(|info| info.ret.as_ref())
    }

    fn enqueue(&mut self, key: FnKey) {
        if self.queued.insert(key) {
            self.worklist.push_back(key);
        }
    }

    /// Join `args` into the entry summary for `key`, enqueueing on change.
    fn join_entry(&mut self, key: FnKey, body: Statement, args: Product<V>) -> Result<(), E> {
        let widen_after = self.widen_after;
        match self.summaries.get_mut(&key) {
            None => {
                self.summaries.insert(
                    key,
                    FnInfo {
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
                let widen = info.entry_joins > widen_after;
                let joined = join_products(&info.entry, &args, widen)?;
                if joined != info.entry {
                    info.entry = joined;
                    self.enqueue(key);
                }
            }
        }
        Ok(())
    }

    fn eval_function(&mut self, key: FnKey) -> Result<(), E> {
        let info = self
            .summaries
            .get(&key)
            .ok_or_else(|| E::from(InterpreterError::Custom("missing function summary")))?;
        let (stage, _) = key;
        let body = info.body;
        let entry = info.entry.clone();

        let previous = self.current.replace(key);
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

        let info = self
            .summaries
            .get_mut(&key)
            .ok_or_else(|| E::from(InterpreterError::Custom("missing function summary")))?;
        if let Some(new_ret) = ret_acc {
            let updated = match info.ret.as_ref() {
                None => new_ret,
                Some(old) => join_products(old, &new_ret, false)?,
            };
            if info.ret.as_ref() != Some(&updated) {
                info.ret = Some(updated);
                let callers: Vec<_> = info.callers.iter().copied().collect();
                for caller in callers {
                    self.enqueue(caller);
                }
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
        match scope.body {
            ScopeBody::Region(region) => {
                let entry = query::region_entry(self.pipeline, stage, region)
                    .map_err(E::from)?
                    .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?;
                self.eval_cfg(stage, env, entry, scope.args, ret_acc)
            }
            ScopeBody::Block(block) => self.eval_cfg(stage, env, block, scope.args, ret_acc),
            ScopeBody::Immediate => {
                join_opt(ret_acc, scope.args, false)?;
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
                        join_opt(ret_acc, values, false)?;
                        break;
                    }
                    Effect::Yield(_) => {
                        return Err(E::from(InterpreterError::UnexpectedYield(statement)));
                    }
                    Effect::Call(call) => self.eval_call(stage, env, call)?,
                    Effect::Enter(scope) => {
                        let results = scope.results.clone();
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

    /// Join edge arguments into a successor's entry state, enqueueing it when
    /// the state changes.
    fn flow(
        &mut self,
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
                let widen = *count > self.widen_after;
                let joined = join_products(old, &args, widen)?;
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

    /// Evaluate a hook-driven structured scope to its local fixpoint.
    /// Returns the joined finish results, or `None` when no path finishes
    /// (all paths return, or the loop never exits).
    fn eval_scope(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
        scope: Scope<V, E>,
        ret_acc: &mut Option<Product<V>>,
    ) -> Result<Option<Product<V>>, E> {
        let block = match scope.body {
            ScopeBody::Immediate => return Ok(Some(scope.args)),
            ScopeBody::Block(block) => block,
            ScopeBody::Region(_) => {
                return Err(E::from(InterpreterError::Custom(
                    "inline region scopes are not supported by the abstract interpreter",
                )));
            }
        };

        let mut entry = scope.args;
        let mut hook = scope.hook;
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
                    join_opt(&mut finish, yielded, false)?;
                    break;
                }
                Some(h) => {
                    let step = h.on_yield(&entry, yielded, &mut EngineEnv { interp: self, env })?;
                    let (args, next_hook) = match step {
                        ScopeStep::Finish(results) => {
                            join_opt(&mut finish, results, false)?;
                            break;
                        }
                        ScopeStep::Repeat { args, hook } => (args, hook),
                        ScopeStep::RepeatOrFinish {
                            args,
                            results,
                            hook,
                        } => {
                            join_opt(&mut finish, results, false)?;
                            (args, hook)
                        }
                    };
                    let widen = iterations > self.widen_after;
                    let joined = join_products(&entry, &args, widen)?;
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
        let results_slots = scopes
            .first()
            .map(|scope| scope.results.clone())
            .unwrap_or_default();
        let mut acc: Option<Product<V>> = None;
        for scope in scopes {
            if let Some(values) = self.eval_scope(stage, env, scope, ret_acc)? {
                join_opt(&mut acc, values, false)?;
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
                    join_opt(ret_acc, values, false)?;
                    return Ok(BodyOutcome::Returned);
                }
                Effect::Call(call) => self.eval_call(stage, env, call)?,
                Effect::Enter(scope) => {
                    let results = scope.results.clone();
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

    /// Summarize a call: join arguments into the callee's entry summary and
    /// read its current return summary (bottom until the callee converges).
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
        let key = (target.stage, target.function);
        self.join_entry(key, target.body, call.args)?;
        if let (Some(info), Some(caller)) = (self.summaries.get_mut(&key), self.current)
            && caller != key
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
fn join_products<V, E>(old: &Product<V>, new: &Product<V>, widen: bool) -> Result<Product<V>, E>
where
    V: Clone + Widen,
    E: From<InterpreterError>,
{
    if old.len() != new.len() {
        return Err(E::from(InterpreterError::ProductArityMismatch {
            expected: old.len(),
            actual: new.len(),
        }));
    }
    Ok(old
        .iter()
        .zip(new.iter())
        .map(
            |(old, new)| {
                if widen { old.widen(new) } else { old.join(new) }
            },
        )
        .collect())
}

/// Join into an optional accumulator (used for return/finish products whose
/// arity is unknown until the first contribution).
fn join_opt<V, E>(acc: &mut Option<Product<V>>, values: Product<V>, widen: bool) -> Result<(), E>
where
    V: Clone + Widen,
    E: From<InterpreterError>,
{
    match acc {
        None => *acc = Some(values),
        Some(old) => *old = join_products(old, &values, widen)?,
    }
    Ok(())
}
