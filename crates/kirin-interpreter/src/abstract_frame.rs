//! Customizable frame-based traversal for the **abstract** engine.
//!
//! This is the abstract analogue of [`frame`](crate::frame): the dialect API
//! still produces a closed [`ForwardEffect`] per statement, and these frames
//! decide how the [`AbstractInterpreter`](crate::AbstractInterpreter)
//! *traverses* — CFG block worklists with join/widen, branch exploration,
//! single-block body walks, and call summarization. The
//! engine just runs a stack of frames (`run_frames`), so a language can supply a
//! custom total frame enum — reusing these standard frames via
//! [`AbstractFrameBuild`] — to observe or replace traversal without forking the
//! engine.
//!
//! The framework owns no structured-control concept: a structured dialect pushes
//! a frame **it owns** ([`ForwardEffect::Push`]), and all loop/branch/alternative
//! policy lives in that dialect frame (it may reuse [`AbstractBlockFrame`] to
//! walk a chosen body). The interprocedural
//! *policy* (summary keying, join/widen, caller recording — including same-key
//! recursion) stays atomic in the engine behind [`AbstractFrameDriver`]; frames
//! only choose what to step next.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{Block, CompileStage, Product, SSAValue, Statement};

use crate::{
    AbstractFrameDriver, CallEffect, EnvIndex, ForwardEffect, ForwardInterp, Frame, FrameEffect,
    InterpreterError,
};

/// Completion payloads produced by the standard abstract frames.
pub enum AbstractCompletion<V> {
    /// A pushed frame finished with these finish values, or `None` if no path
    /// through it finished (e.g. it returned).
    Finished(Option<Product<V>>),
    /// The whole function body has been analyzed. The return product is in the
    /// engine's accumulator; this only signals the inner driver loop to stop.
    FunctionDone,
}

/// Construction trait letting any total abstract frame enum embed the standard
/// abstract frames (the analogue of [`FrameBuild`](crate::FrameBuild)).
pub trait AbstractFrameBuild<V, E, K>: Sized {
    fn from_function(frame: AbstractFunctionFrame<V, E, K>) -> Self;
    fn from_cfg(frame: AbstractCfgFrame<V, E, K>) -> Self;
    fn from_block(frame: AbstractBlockFrame<V, E, K>) -> Self;
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self;
}

// ===========================================================================
// Function-entry frame (root of one function evaluation)
// ===========================================================================

/// Root frame of one function evaluation: build the entry [`FunctionBody`](crate::FunctionBody)
/// and descend into the body CFG.
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
        let body = interp.enter_function(self.stage, self.body, self.args, self.env)?;
        let entry = interp
            .region_entry(self.stage, body.region)?
            .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?;
        let cfg = AbstractCfgFrame::enter(self.stage, self.env, entry, body.args);
        Ok(FrameEffect::Push {
            parent: F::from_function(Self {
                args: Product::new(),
                ..self
            }),
            child: F::from_cfg(cfg),
        })
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
            "function frame resumed with a body completion",
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
    /// Result slots awaiting a pushed frame's completion.
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
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K> + ForwardInterp<Frame = F>,
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
            ForwardEffect::Next => Ok(FrameEffect::Continue(F::from_cfg(self))),
            ForwardEffect::Jump(edge) => {
                self.flow(interp, edge.target, edge.args)?;
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            ForwardEffect::Branch(edges) => {
                for edge in edges {
                    self.flow(interp, edge.target, edge.args)?;
                }
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            ForwardEffect::Return(values) => {
                interp.contribute_return(values)?;
                self.current = None;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            ForwardEffect::Yield(_) => Err(E::from(InterpreterError::UnexpectedYield(statement))),
            ForwardEffect::Call(call) => {
                let call_frame = AbstractCallFrame::new(self.stage, call, self.env);
                Ok(FrameEffect::Push {
                    parent: F::from_cfg(self),
                    child: F::from_call(call_frame),
                })
            }
            ForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_cfg(self),
                    child: frame,
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
            AbstractCompletion::Finished(Some(values)) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom("cfg resume without result slots"))
                })?;
                interp.write_results(self.env, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            AbstractCompletion::Finished(None) => {
                // No path through the pushed frame finished: this block path is done.
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
// Block frame: walk one body block (scf-style), completing on yield
// ===========================================================================

/// Evaluate one structured body block: walk it once and, on yield, complete
/// with the yielded product. Loop/branch policy is **not** here — a dialect's
/// own frame re-pushes a block frame to iterate. Nested pushes/calls are driven
/// like the CFG frame.
pub struct AbstractBlockFrame<V, E, K> {
    stage: CompileStage,
    env: EnvIndex,
    block: Block,
    cursor: Option<Statement>,
    /// Entry arguments not yet bound — bound on the first step, so building the
    /// frame needs no engine access (see [`BodyFrame`](crate::BodyFrame)).
    pending: Option<Product<V>>,
    resume_slots: Option<Product<SSAValue>>,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractBlockFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    /// A block frame that binds its entry parameters on the first step. Pure
    /// construction — needs no engine access.
    pub fn new(stage: CompileStage, env: EnvIndex, block: Block, args: Product<V>) -> Self {
        Self {
            stage,
            env,
            block,
            cursor: None,
            pending: Some(args),
            resume_slots: None,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K> + ForwardInterp<Frame = F>,
        F: AbstractFrameBuild<V, E, K>,
    {
        // Bind entry arguments lazily on the first step.
        if let Some(args) = self.pending.take() {
            interp.bind_block_args(self.stage, self.env, self.block, &args)?;
            self.cursor = interp.first_statement(self.stage, self.block)?;
            return Ok(FrameEffect::Continue(F::from_block(self)));
        }
        let Some(statement) = self.cursor else {
            return Err(E::from(InterpreterError::BlockFellThrough(self.block)));
        };
        self.cursor = interp.next_statement(self.stage, self.block, statement)?;

        match interp.run_statement(self.stage, statement, self.env)? {
            ForwardEffect::Next => Ok(FrameEffect::Continue(F::from_block(self))),
            ForwardEffect::Yield(values) => Ok(FrameEffect::Complete(
                AbstractCompletion::Finished(Some(values)),
            )),
            ForwardEffect::Return(values) => {
                interp.contribute_return(values)?;
                Ok(FrameEffect::Complete(AbstractCompletion::Finished(None)))
            }
            ForwardEffect::Call(call) => {
                let call_frame = AbstractCallFrame::new(self.stage, call, self.env);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: F::from_call(call_frame),
                })
            }
            ForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: frame,
                })
            }
            ForwardEffect::Jump(_) | ForwardEffect::Branch(_) => Err(E::from(
                InterpreterError::Custom("CFG transfer inside a structured body block"),
            )),
        }
    }

    /// A pushed call frame finished: continue walking the body.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, AbstractCompletion<V>>
    where
        F: AbstractFrameBuild<V, E, K>,
    {
        FrameEffect::Continue(F::from_block(self))
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
            AbstractCompletion::Finished(Some(values)) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "block resume without result slots",
                    ))
                })?;
                interp.write_results(self.env, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_block(self)))
            }
            // A nested push returned without finishing: this body pass left via
            // return, so the block completes without a finish value.
            AbstractCompletion::Finished(None) => {
                Ok(FrameEffect::Complete(AbstractCompletion::Finished(None)))
            }
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "block frame resumed with a function completion",
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

/// The default total abstract frame enum: standard abstract traversal (no
/// structured-control dialect frames). A language adding such a dialect defines
/// its own enum reusing these via [`AbstractFrameBuild`].
pub enum StandardAbstractFrame<V, E, K> {
    Function(AbstractFunctionFrame<V, E, K>),
    Cfg(AbstractCfgFrame<V, E, K>),
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
}

impl<V, E, K> AbstractFrameBuild<V, E, K> for StandardAbstractFrame<V, E, K> {
    fn from_function(frame: AbstractFunctionFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Function(frame)
    }
    fn from_cfg(frame: AbstractCfgFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Cfg(frame)
    }
    fn from_block(frame: AbstractBlockFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Block(frame)
    }
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Call(frame)
    }
}

impl<I, V, E, K> Frame<I> for StandardAbstractFrame<V, E, K>
where
    I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>
        + ForwardInterp<Frame = StandardAbstractFrame<V, E, K>>,
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    type Completion = AbstractCompletion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Function(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Cfg(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Block(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Call(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Function(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardAbstractFrame::Cfg(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardAbstractFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
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
            StandardAbstractFrame::Block(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardAbstractFrame::Call(frame) => frame.resume_into::<Self>(completion),
        }
    }
}
