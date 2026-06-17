//! Customizable frame-based traversal for the **abstract** engine.
//!
//! This is the abstract analogue of [`frame`](crate::frame): the dialect API
//! still produces a closed [`Effect`] per statement, and these frames decide how
//! the [`AbstractInterpreter`](crate::AbstractInterpreter) *traverses* — CFG
//! block worklists with join/widen, branch exploration, hook-driven scope
//! fixpoints, undecided scope alternatives, and call summarization. The engine
//! just runs a stack of frames (`run_frames`), so a compiler/analysis author can
//! supply a custom total frame enum — reusing these standard frames via
//! [`AbstractFrameBuild`] — to observe or replace traversal without forking the
//! engine.
//!
//! Frames are *traversal*, never dialect semantics: [`Scope`]/[`ScopeHook`] are
//! consumed here, not replaced. The interprocedural *policy* (summary keying,
//! join/widen, caller recording — including same-key recursion) stays atomic in
//! the engine behind [`AbstractFrameDriver`]; frames only choose what to step
//! next.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{Block, CompileStage, Product, SSAValue, Statement};

use crate::ctx::EngineEnv;
use crate::{
    AbstractFrameDriver, CallEffect, Effect, EnvIndex, Frame, FrameEffect, InterpreterError, Scope,
    ScopeBody, ScopeStep,
};

/// Completion payloads produced by the standard abstract frames.
///
/// `#[non_exhaustive]` so custom abstract frames can add payloads later.
#[non_exhaustive]
pub enum AbstractCompletion<V> {
    /// A scope (or scope-alternatives) reached its local fixpoint with these
    /// joined finish results, or `None` if no path through it finished.
    ScopeFinished(Option<Product<V>>),
    /// The whole function body has been analyzed. The return product is in the
    /// engine's accumulator; this only signals the inner driver loop to stop.
    FunctionDone,
}

/// Construction trait letting any total abstract frame enum embed the standard
/// abstract frames (the analogue of [`FrameBuild`](crate::FrameBuild)).
pub trait AbstractFrameBuild<V, E, K>: Sized {
    fn from_function(frame: AbstractFunctionFrame<V, E, K>) -> Self;
    fn from_cfg(frame: AbstractCfgFrame<V, E, K>) -> Self;
    fn from_scope(frame: AbstractScopeFrame<V, E, K>) -> Self;
    fn from_scope_alternatives(frame: AbstractScopeAlternativesFrame<V, E, K>) -> Self;
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self;
}

// ===========================================================================
// Function-entry frame (root of one function evaluation)
// ===========================================================================

/// Root frame of one function evaluation: build the entry [`Scope`] and descend
/// into the body CFG (or contribute an immediate return).
pub struct AbstractFunctionFrame<V, E, K> {
    stage: CompileStage,
    body: Statement,
    args: Product<V>,
    env: EnvIndex,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractFunctionFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    pub fn new(stage: CompileStage, body: Statement, args: Product<V>, env: EnvIndex) -> Self {
        Self {
            stage,
            body,
            args,
            env,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        let scope = interp.enter_function(self.stage, self.body, self.args, self.env)?;
        let body = scope.body();
        let Scope { args, .. } = scope;
        match body {
            ScopeBody::Region(region) => {
                let entry = interp
                    .region_entry(self.stage, region)?
                    .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?;
                let cfg = AbstractCfgFrame::enter(self.stage, self.env, entry, args);
                Ok(FrameEffect::Push {
                    parent: F::from_function(Self {
                        args: Product::new(),
                        ..self
                    }),
                    child: F::from_cfg(cfg),
                })
            }
            ScopeBody::Block(block) => {
                let cfg = AbstractCfgFrame::enter(self.stage, self.env, block, args);
                Ok(FrameEffect::Push {
                    parent: F::from_function(Self {
                        args: Product::new(),
                        ..self
                    }),
                    child: F::from_cfg(cfg),
                })
            }
            ScopeBody::Immediate => {
                interp.contribute_return(args)?;
                Ok(FrameEffect::Complete(AbstractCompletion::FunctionDone))
            }
        }
    }

    /// The body CFG drained: the function is fully analyzed.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, AbstractCompletion<V>> {
        FrameEffect::Complete(AbstractCompletion::FunctionDone)
    }

    pub fn resume_into<F>(
        self,
        _completion: AbstractCompletion<V>,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "function frame resumed with a scope completion",
        )))
    }
}

// ===========================================================================
// CFG frame: block worklist with join/widen at merge points
// ===========================================================================

/// Worklist evaluation of a function-body CFG. Block parameters join across
/// incoming edges and widen after the analysis threshold; one statement is
/// stepped per `step` so a wrapping frame observes every move.
pub struct AbstractCfgFrame<V, E, K> {
    stage: CompileStage,
    env: EnvIndex,
    block_in: HashMap<Block, Product<V>>,
    visits: HashMap<Block, usize>,
    pending: VecDeque<Block>,
    queued: HashSet<Block>,
    /// The block currently being walked, plus its statement cursor.
    current: Option<(Block, Option<Statement>)>,
    iterations: usize,
    /// Result slots awaiting a pushed scope/alternatives completion.
    resume_slots: Option<Product<SSAValue>>,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractCfgFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    pub fn enter(stage: CompileStage, env: EnvIndex, entry: Block, args: Product<V>) -> Self {
        let mut block_in = HashMap::new();
        let mut pending = VecDeque::new();
        let mut queued = HashSet::new();
        block_in.insert(entry, args);
        pending.push_back(entry);
        queued.insert(entry);
        Self {
            stage,
            env,
            block_in,
            visits: HashMap::new(),
            pending,
            queued,
            current: None,
            iterations: 0,
            resume_slots: None,
            _marker: PhantomData,
        }
    }

    /// Join `args` into successor `target`'s entry state via the analysis,
    /// enqueueing it when the state changes (the CFG's per-target `visits`
    /// counter selects join vs. widen).
    fn flow<I>(&mut self, interp: &mut I, target: Block, args: Product<V>) -> Result<(), E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
    {
        let changed = match self.block_in.get_mut(&target) {
            None => {
                self.block_in.insert(target, args);
                true
            }
            Some(old) => {
                let count = self.visits.entry(target).or_insert(0);
                *count += 1;
                let joined = interp.analysis_merge(old, &args, *count)?;
                if joined != *old {
                    *old = joined;
                    true
                } else {
                    false
                }
            }
        };
        if changed && self.queued.insert(target) {
            self.pending.push_back(target);
        }
        Ok(())
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        let (block, cursor) = match self.current.take() {
            // No current block: start the next pending one (bind its entry).
            None => {
                let Some(block) = self.pending.pop_front() else {
                    // Worklist drained: the function body is fully analyzed.
                    return Ok(FrameEffect::Done);
                };
                self.queued.remove(&block);
                self.iterations += 1;
                if self.iterations > interp.max_iterations() {
                    return Err(E::from(InterpreterError::FixpointDiverged));
                }
                let in_args = self.block_in.get(&block).cloned().ok_or_else(|| {
                    E::from(InterpreterError::Custom("missing block entry state"))
                })?;
                interp.bind_block_args(self.stage, self.env, block, &in_args)?;
                let cursor = interp.first_statement(self.stage, block)?;
                self.current = Some((block, cursor));
                return Ok(FrameEffect::Continue(F::from_cfg(self)));
            }
            Some(pos) => pos,
        };

        let Some(statement) = cursor else {
            // Block ran out of statements without a transfer: end this path.
            return Ok(FrameEffect::Continue(F::from_cfg(self)));
        };
        let next = interp.next_statement(self.stage, block, statement)?;
        self.current = Some((block, next));

        match interp.run_statement(self.stage, statement, self.env)? {
            Effect::Next => Ok(FrameEffect::Continue(F::from_cfg(self))),
            Effect::Jump(edge) => {
                self.flow(interp, edge.target, edge.args)?;
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            Effect::Branch(edges) => {
                for edge in edges {
                    self.flow(interp, edge.target, edge.args)?;
                }
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            Effect::Return(values) => {
                interp.contribute_return(values)?;
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            Effect::Yield(_) => Err(E::from(InterpreterError::UnexpectedYield(statement))),
            Effect::Call(call) => {
                let call_frame = AbstractCallFrame::new(self.stage, call, self.env);
                Ok(FrameEffect::Push {
                    parent: F::from_cfg(self),
                    child: F::from_call(call_frame),
                })
            }
            Effect::Enter(scope) => {
                let slots = scope.results.clone();
                if matches!(scope.body(), ScopeBody::Immediate) {
                    let Scope { args, .. } = scope;
                    interp.write_results(self.env, &slots, args)?;
                    Ok(FrameEffect::Continue(F::from_cfg(self)))
                } else {
                    self.resume_slots = Some(slots);
                    let child = AbstractScopeFrame::enter(interp, self.stage, self.env, scope)?;
                    Ok(FrameEffect::Push {
                        parent: F::from_cfg(self),
                        child: F::from_scope(child),
                    })
                }
            }
            Effect::EnterAny(scopes) => {
                let slots = scopes
                    .first()
                    .map(|scope| scope.results.clone())
                    .unwrap_or_default();
                self.resume_slots = Some(slots);
                let child = AbstractScopeAlternativesFrame::new(self.stage, self.env, scopes);
                Ok(FrameEffect::Push {
                    parent: F::from_cfg(self),
                    child: F::from_scope_alternatives(child),
                })
            }
        }
    }

    /// A pushed call frame finished (results already written): continue the
    /// current block's statement walk.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, AbstractCompletion<V>>
    where
        F: AbstractFrameBuild<V, E, K>,
    {
        FrameEffect::Continue(F::from_cfg(self))
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        match completion {
            AbstractCompletion::ScopeFinished(Some(values)) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom("cfg resume without result slots"))
                })?;
                interp.write_results(self.env, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            AbstractCompletion::ScopeFinished(None) => {
                // No path through the scope finished: this block path is done.
                self.resume_slots = None;
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "cfg frame resumed with a function completion",
            ))),
        }
    }
}

// ===========================================================================
// Scope frame: hook-driven structured scope fixpoint (folds the body walk)
// ===========================================================================

/// Evaluate one hook-driven structured scope to its local fixpoint: walk the
/// body block, and on yield run the [`ScopeHook`](crate::ScopeHook) to decide
/// finish vs. (joined/widened) re-entry until the entry state is stable.
pub struct AbstractScopeFrame<V, E, K> {
    stage: CompileStage,
    env: EnvIndex,
    block: Block,
    entry: Product<V>,
    hook: Option<Box<dyn crate::ScopeHook<V, E>>>,
    finish: Option<Product<V>>,
    iterations: usize,
    cursor: Option<Statement>,
    resume_slots: Option<Product<SSAValue>>,
    _marker: PhantomData<fn() -> K>,
}

impl<V, E, K> AbstractScopeFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    /// Build a scope frame for a single-block scope and bind its first body pass.
    pub fn enter<I>(
        interp: &mut I,
        stage: CompileStage,
        env: EnvIndex,
        scope: Scope<V, E>,
    ) -> Result<Self, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
    {
        let block = match scope.body() {
            ScopeBody::Block(block) => block,
            ScopeBody::Immediate => {
                return Err(E::from(InterpreterError::Custom(
                    "immediate scope must be finished by the caller",
                )));
            }
            ScopeBody::Region(_) => {
                return Err(E::from(InterpreterError::Custom(
                    "inline region scopes are not supported by the abstract interpreter",
                )));
            }
        };
        let Scope { args, hook, .. } = scope;
        interp.bind_block_args(stage, env, block, &args)?;
        let cursor = interp.first_statement(stage, block)?;
        Ok(Self {
            stage,
            env,
            block,
            entry: args,
            hook,
            finish: None,
            iterations: 1,
            cursor,
            resume_slots: None,
            _marker: PhantomData,
        })
    }

    /// Join a finish/contribution into the scope's result accumulator (never widens).
    fn join_finish<I>(&mut self, interp: &mut I, values: Product<V>) -> Result<(), E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
    {
        let merged = match self.finish.take() {
            None => values,
            Some(current) => interp.analysis_merge(&current, &values, 0)?,
        };
        self.finish = Some(merged);
        Ok(())
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        let Some(statement) = self.cursor else {
            return Err(E::from(InterpreterError::BlockFellThrough(self.block)));
        };
        self.cursor = interp.next_statement(self.stage, self.block, statement)?;

        match interp.run_statement(self.stage, statement, self.env)? {
            Effect::Next => Ok(FrameEffect::Continue(F::from_scope(self))),
            Effect::Yield(values) => self.on_yield::<I, F>(interp, values),
            Effect::Return(values) => {
                interp.contribute_return(values)?;
                Ok(FrameEffect::Complete(AbstractCompletion::ScopeFinished(
                    self.finish,
                )))
            }
            Effect::Call(call) => {
                let call_frame = AbstractCallFrame::new(self.stage, call, self.env);
                Ok(FrameEffect::Push {
                    parent: F::from_scope(self),
                    child: F::from_call(call_frame),
                })
            }
            Effect::Enter(scope) => {
                let slots = scope.results.clone();
                if matches!(scope.body(), ScopeBody::Immediate) {
                    let Scope { args, .. } = scope;
                    interp.write_results(self.env, &slots, args)?;
                    Ok(FrameEffect::Continue(F::from_scope(self)))
                } else {
                    self.resume_slots = Some(slots);
                    let child = AbstractScopeFrame::enter(interp, self.stage, self.env, scope)?;
                    Ok(FrameEffect::Push {
                        parent: F::from_scope(self),
                        child: F::from_scope(child),
                    })
                }
            }
            Effect::EnterAny(scopes) => {
                let slots = scopes
                    .first()
                    .map(|scope| scope.results.clone())
                    .unwrap_or_default();
                self.resume_slots = Some(slots);
                let child = AbstractScopeAlternativesFrame::new(self.stage, self.env, scopes);
                Ok(FrameEffect::Push {
                    parent: F::from_scope(self),
                    child: F::from_scope_alternatives(child),
                })
            }
            Effect::Jump(_) | Effect::Branch(_) => Err(E::from(InterpreterError::Custom(
                "CFG transfer inside a structured scope body",
            ))),
        }
    }

    fn on_yield<I, F>(
        mut self,
        interp: &mut I,
        values: Product<V>,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        let (args, next_hook) = match self.hook.take() {
            None => {
                self.join_finish(interp, values)?;
                return Ok(FrameEffect::Complete(AbstractCompletion::ScopeFinished(
                    self.finish,
                )));
            }
            Some(hook) => {
                let step = {
                    let mut env = EngineEnv {
                        interp: &mut *interp,
                        env: self.env,
                    };
                    hook.on_yield(&self.entry, values, &mut env)?
                };
                match step {
                    ScopeStep::Finish(results) => {
                        self.join_finish(interp, results)?;
                        return Ok(FrameEffect::Complete(AbstractCompletion::ScopeFinished(
                            self.finish,
                        )));
                    }
                    ScopeStep::Repeat { args, hook } => (args, hook),
                    ScopeStep::RepeatOrFinish {
                        args,
                        results,
                        hook,
                    } => {
                        self.join_finish(interp, results)?;
                        (args, hook)
                    }
                }
            }
        };
        let joined = interp.analysis_merge(&self.entry, &args, self.iterations)?;
        if joined == self.entry {
            // Stable entry state: re-running the body adds nothing.
            return Ok(FrameEffect::Complete(AbstractCompletion::ScopeFinished(
                self.finish,
            )));
        }
        self.entry = joined;
        self.hook = Some(next_hook);
        self.iterations += 1;
        if self.iterations > interp.max_iterations() {
            return Err(E::from(InterpreterError::FixpointDiverged));
        }
        interp.bind_block_args(self.stage, self.env, self.block, &self.entry)?;
        self.cursor = interp.first_statement(self.stage, self.block)?;
        Ok(FrameEffect::Continue(F::from_scope(self)))
    }

    /// A pushed call frame finished: continue walking the body.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, AbstractCompletion<V>>
    where
        F: AbstractFrameBuild<V, E, K>,
    {
        FrameEffect::Continue(F::from_scope(self))
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        match completion {
            AbstractCompletion::ScopeFinished(Some(values)) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "scope resume without result slots",
                    ))
                })?;
                interp.write_results(self.env, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_scope(self)))
            }
            AbstractCompletion::ScopeFinished(None) => {
                // A nested scope returned without finishing: this body pass left
                // via return, so the scope completes with its finish accumulator.
                Ok(FrameEffect::Complete(AbstractCompletion::ScopeFinished(
                    self.finish,
                )))
            }
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "scope frame resumed with a function completion",
            ))),
        }
    }
}

// ===========================================================================
// Scope-alternatives frame: undecided structured branch (EnterAny)
// ===========================================================================

/// Evaluate each alternative scope and join their finish results. The parent
/// writes the joined product into the shared result slots.
pub struct AbstractScopeAlternativesFrame<V, E, K> {
    stage: CompileStage,
    env: EnvIndex,
    remaining: VecDeque<Scope<V, E>>,
    acc: Option<Product<V>>,
    _marker: PhantomData<fn() -> K>,
}

impl<V, E, K> AbstractScopeAlternativesFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    pub fn new(stage: CompileStage, env: EnvIndex, scopes: Vec<Scope<V, E>>) -> Self {
        Self {
            stage,
            env,
            remaining: scopes.into(),
            acc: None,
            _marker: PhantomData,
        }
    }

    fn join_acc<I>(&mut self, interp: &mut I, values: Product<V>) -> Result<(), E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
    {
        let merged = match self.acc.take() {
            None => values,
            Some(current) => interp.analysis_merge(&current, &values, 0)?,
        };
        self.acc = Some(merged);
        Ok(())
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        let Some(scope) = self.remaining.pop_front() else {
            return Ok(FrameEffect::Complete(AbstractCompletion::ScopeFinished(
                self.acc,
            )));
        };
        if matches!(scope.body(), ScopeBody::Immediate) {
            let Scope { args, .. } = scope;
            self.join_acc(interp, args)?;
            Ok(FrameEffect::Continue(F::from_scope_alternatives(self)))
        } else {
            let child = AbstractScopeFrame::enter(interp, self.stage, self.env, scope)?;
            Ok(FrameEffect::Push {
                parent: F::from_scope_alternatives(self),
                child: F::from_scope(child),
            })
        }
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "scope-alternatives frame resumed without a scope completion",
        )))
    }

    pub fn resume_into<I, F>(
        mut self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        match completion {
            AbstractCompletion::ScopeFinished(Some(values)) => {
                self.join_acc(interp, values)?;
                Ok(FrameEffect::Continue(F::from_scope_alternatives(self)))
            }
            // This alternative did not finish: skip it and try the next.
            AbstractCompletion::ScopeFinished(None) => {
                Ok(FrameEffect::Continue(F::from_scope_alternatives(self)))
            }
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "scope-alternatives frame resumed with a function completion",
            ))),
        }
    }
}

// ===========================================================================
// Call frame: summarize a call (no descent — the interprocedural fixpoint
// re-evaluates the callee).
// ===========================================================================

/// Summarize one call through the engine's interprocedural protocol, then
/// finish (results are written by `summarize_call`).
pub struct AbstractCallFrame<V, E, K> {
    stage: CompileStage,
    call: CallEffect<V>,
    env: EnvIndex,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractCallFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    pub fn new(stage: CompileStage, call: CallEffect<V>, env: EnvIndex) -> Self {
        Self {
            stage,
            call,
            env,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        interp.summarize_call(self.stage, self.call, self.env)?;
        Ok(FrameEffect::Done)
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "call frame resumed without a return",
        )))
    }

    pub fn resume_into<F>(
        self,
        _completion: AbstractCompletion<V>,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "call frame resumed with a completion",
        )))
    }
}

// ===========================================================================
// The default total abstract frame enum
// ===========================================================================

/// The default total abstract frame enum: standard abstract traversal. A custom
/// analysis can define its own enum reusing these via [`AbstractFrameBuild`].
pub enum StandardAbstractFrame<V, E, K> {
    Function(AbstractFunctionFrame<V, E, K>),
    Cfg(AbstractCfgFrame<V, E, K>),
    Scope(AbstractScopeFrame<V, E, K>),
    Alternatives(AbstractScopeAlternativesFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
}

impl<V, E, K> AbstractFrameBuild<V, E, K> for StandardAbstractFrame<V, E, K> {
    fn from_function(frame: AbstractFunctionFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Function(frame)
    }
    fn from_cfg(frame: AbstractCfgFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Cfg(frame)
    }
    fn from_scope(frame: AbstractScopeFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Scope(frame)
    }
    fn from_scope_alternatives(frame: AbstractScopeAlternativesFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Alternatives(frame)
    }
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Call(frame)
    }
}

impl<I> Frame<I> for StandardAbstractFrame<I::Value, I::Error, I::SummaryKey>
where
    I: AbstractFrameDriver,
    I::Value: Clone + PartialEq,
    I::Error: From<InterpreterError>,
    I::SummaryKey: Clone + Eq + Hash,
{
    type Completion = AbstractCompletion<I::Value>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Function(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Cfg(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Scope(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Alternatives(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Call(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Function(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardAbstractFrame::Cfg(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardAbstractFrame::Scope(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardAbstractFrame::Alternatives(frame) => frame.resume_done_into::<Self>(),
            StandardAbstractFrame::Call(frame) => frame.resume_done_into::<Self>(),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Function(frame) => frame.resume_into::<Self>(completion),
            StandardAbstractFrame::Cfg(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardAbstractFrame::Scope(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardAbstractFrame::Alternatives(frame) => {
                frame.resume_into::<I, Self>(completion, interp)
            }
            StandardAbstractFrame::Call(frame) => frame.resume_into::<Self>(completion),
        }
    }
}
