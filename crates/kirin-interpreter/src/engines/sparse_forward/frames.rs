//! Customizable frame-based traversal for the **abstract** engine.
//!
//! This is the abstract analogue of [`frame`](crate::core::frame): the dialect API
//! still produces a closed [`SparseForwardEffect`] per statement, and these frames
//! decide how the [`SparseForwardInterpreter`](crate::SparseForwardInterpreter)
//! *traverses* — CFG block worklists with join/widen, branch exploration,
//! single-block body walks, and call summarization. The
//! engine just runs a stack of frames (`run_frames`), so a language can supply a
//! custom total frame enum — reusing these standard frames via
//! [`AbstractFrameBuild`] — to observe or replace traversal without forking the
//! engine.
//!
//! The framework owns no structured-control concept: a structured dialect pushes
//! a frame **it owns** ([`SparseForwardEffect::Push`]), and all loop/branch/alternative
//! policy lives in that dialect frame (it may reuse [`AbstractBlockFrame`] to
//! walk a chosen body). The interprocedural
//! *policy* (summary keying, join/widen, caller recording — including same-key
//! recursion) stays atomic in the engine behind [`AbstractFrameDriver`]; frames
//! only choose what to step next.

use std::collections::VecDeque;
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{Block, CompileStage, DiGraph, Product, SSAValue, Statement};

use crate::{
    AbstractFrameDriver, Body, CallEffect, Edge, EnvIndex, Frame, FrameEffect, InterpreterError,
    SparseForwardEffect, SparseForwardInterp,
};

/// Completion payloads produced by the standard abstract frames.
pub enum AbstractCompletion<V> {
    /// A pushed frame finished with these finish values, or `None` if no path
    /// through it finished (e.g. it returned).
    Finished(Option<Product<V>>),
    /// The whole function body has been analyzed. The return product is in the
    /// engine's accumulator; this only signals the inner driver loop to stop.
    FunctionDone,
    /// A single CFG **block owner** finished its one-pass walk (M3b): the outgoing
    /// CFG edges it took (empty for a `return`). Any return value was contributed
    /// to the engine's return accumulator during the walk.
    CFGBlock { edges: Vec<Edge<V>> },
}

/// Construction trait letting any total abstract frame enum embed the standard
/// abstract frames (the analogue of [`FrameBuild`](crate::FrameBuild)).
pub trait AbstractFrameBuild<V, E, K>: Sized {
    fn from_block(frame: AbstractBlockFrame<V, E, K>) -> Self;
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self;

    /// Embed the standard abstract digraph walker.
    ///
    /// Graph bodies are opt-in: a total abstract frame enum that carries no
    /// [`AbstractDiGraphFrame`] inherits this rejection rather than pretending
    /// to analyze one, the same way
    /// [`FrameBuild::from_ungraph_entry`](crate::FrameBuild::from_ungraph_entry)
    /// rejects a callable `UnGraph` without a compiler-supplied policy.
    fn from_digraph(frame: AbstractDiGraphFrame<V, E, K>) -> Result<Self, E>
    where
        E: From<InterpreterError>,
    {
        Err(E::from(InterpreterError::NoDefaultWalker(Body::DiGraph(
            frame.graph(),
        ))))
    }
}

// ===========================================================================
// Block frame: walk one body block (scf-style), completing on yield
// ===========================================================================

/// Evaluate one structured body block: walk it once and, on yield, complete
/// with the yielded product. Loop/branch policy is **not** here — a dialect's
/// own frame re-pushes a block frame to iterate. Nested pushes/calls are driven
/// like the CFG frame.
/// How an [`AbstractBlockFrame`] treats a block terminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockMode {
    /// A structured (scf) body block: completes on `Yield`; `Jump`/`Branch` are an
    /// error. Used by dialect frames that walk a chosen body.
    StructuredBody,
    /// A CFG **block owner** (M3b): completes on `Jump`/`Branch`
    /// ([`AbstractCompletion::CFGBlock`]) or `Return`; `Yield` is an error.
    CFGBlock,
}

pub struct AbstractBlockFrame<V, E, K> {
    stage: CompileStage,
    index: EnvIndex,
    block: Block,
    cursor: Option<Statement>,
    mode: BlockMode,
    /// Entry arguments not yet bound — bound on the first step, so building the
    /// frame needs no engine access (see [`BlockFrame`](crate::BlockFrame)).
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
    /// A structured-body block frame (completes on `Yield`). Binds its entry
    /// parameters on the first step. Pure construction — needs no engine access.
    pub fn new(stage: CompileStage, index: EnvIndex, block: Block, args: Product<V>) -> Self {
        Self::with_mode(stage, index, block, args, BlockMode::StructuredBody)
    }

    /// A CFG block-owner frame (M3b): completes on `Jump`/`Branch`/`Return`.
    pub fn new_cfg_block(
        stage: CompileStage,
        index: EnvIndex,
        block: Block,
        args: Product<V>,
    ) -> Self {
        Self::with_mode(stage, index, block, args, BlockMode::CFGBlock)
    }

    fn with_mode(
        stage: CompileStage,
        index: EnvIndex,
        block: Block,
        args: Product<V>,
        mode: BlockMode,
    ) -> Self {
        Self {
            stage,
            index,
            block,
            cursor: None,
            mode,
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
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>
            + SparseForwardInterp<Frame = F>,
        F: AbstractFrameBuild<V, E, K>,
    {
        // Bind entry arguments lazily on the first step.
        if let Some(args) = self.pending.take() {
            interp.bind_block_args(self.stage, self.index, self.block, &args)?;
            self.cursor = interp.first_statement(self.stage, self.block)?;
            return Ok(FrameEffect::Continue(F::from_block(self)));
        }
        let Some(statement) = self.cursor else {
            return Err(E::from(InterpreterError::BlockFellThrough(self.block)));
        };
        self.cursor = interp.next_statement(self.stage, self.block, statement)?;

        match interp.run_statement(self.stage, statement, self.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(F::from_block(self))),
            SparseForwardEffect::Return(values) => {
                // A return contributes to the function's return accumulator in
                // either mode; the block/body then completes.
                interp.contribute_return(values)?;
                match self.mode {
                    BlockMode::StructuredBody => {
                        Ok(FrameEffect::Complete(AbstractCompletion::Finished(None)))
                    }
                    BlockMode::CFGBlock => {
                        Ok(FrameEffect::Complete(AbstractCompletion::CFGBlock {
                            edges: Vec::new(),
                        }))
                    }
                }
            }
            SparseForwardEffect::Yield(values) => match self.mode {
                BlockMode::StructuredBody => Ok(FrameEffect::Complete(
                    AbstractCompletion::Finished(Some(values)),
                )),
                BlockMode::CFGBlock => Err(E::from(InterpreterError::Custom(
                    "yield inside a CFG block owner",
                ))),
            },
            SparseForwardEffect::Jump(edge) => match self.mode {
                BlockMode::CFGBlock => Ok(FrameEffect::Complete(AbstractCompletion::CFGBlock {
                    edges: vec![edge],
                })),
                BlockMode::StructuredBody => Err(E::from(InterpreterError::Custom(
                    "CFG transfer inside a structured body block",
                ))),
            },
            SparseForwardEffect::Branch(edges) => match self.mode {
                BlockMode::CFGBlock => Ok(FrameEffect::Complete(AbstractCompletion::CFGBlock {
                    edges,
                })),
                BlockMode::StructuredBody => Err(E::from(InterpreterError::Custom(
                    "CFG transfer inside a structured body block",
                ))),
            },
            SparseForwardEffect::Call(call) => {
                let call_frame = AbstractCallFrame::new(self.stage, call, self.index);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: F::from_call(call_frame),
                })
            }
            SparseForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: frame,
                })
            }
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
                interp.write_results(self.index, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_block(self)))
            }
            // A nested push returned without finishing: this pass left via return.
            // A structured body completes without a finish value; a CFG block owner
            // completes as a returning block (no outgoing edges).
            AbstractCompletion::Finished(None) => match self.mode {
                BlockMode::StructuredBody => {
                    Ok(FrameEffect::Complete(AbstractCompletion::Finished(None)))
                }
                BlockMode::CFGBlock => Ok(FrameEffect::Complete(AbstractCompletion::CFGBlock {
                    edges: Vec::new(),
                })),
            },
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "block frame resumed with a function completion",
            ))),
            AbstractCompletion::CFGBlock { .. } => Err(E::from(InterpreterError::Custom(
                "block frame resumed with a CFG-block completion",
            ))),
        }
    }
}

// ===========================================================================
// DiGraph frame: one dependency-ordered pass over a graph body
// ===========================================================================

/// Abstract walker for a [`DiGraph`] body: bind the boundary ports, run the
/// node statements in dependency (topological) order, and complete
/// [`Finished`](AbstractCompletion::Finished) with the graph's declared yields.
///
/// A single pass is **exact** for a DAG — there is no loop inside the graph, so
/// no widening happens here. Convergence pressure comes only from *outside*:
/// the owner's entry product is widened at
/// [`Owner`](crate::Owner) entry when a new call site raises it, and the whole
/// pass is re-run.
///
/// The one substantive difference from the concrete
/// [`DiGraphFrame`](crate::DiGraphFrame) is call handling: a `Call` effect
/// pushes an [`AbstractCallFrame`], so the call goes through the engine's
/// interprocedural summarization protocol (`summarize_call`) instead of
/// descending into the callee. Descending would neither widen nor terminate on
/// recursion.
///
/// Like the other frames, construction is pure — the walk plan is fetched and
/// the ports are bound on the first `step`, so a dialect frame can build one
/// without engine access.
pub struct AbstractDiGraphFrame<V, E, K> {
    stage: CompileStage,
    index: EnvIndex,
    graph: DiGraph,
    /// Entry arguments not yet bound (bound on the first `step`).
    pending: Option<Product<V>>,
    /// Remaining schedule in dependency order; `None` until the first step.
    schedule: Option<VecDeque<Statement>>,
    yields: Vec<SSAValue>,
    /// Result slots awaiting a pushed child frame's completion values.
    resume_slots: Option<Product<SSAValue>>,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractDiGraphFrame<V, E, K> {
    /// The graph body this frame walks. Available without the walking bounds so
    /// [`AbstractFrameBuild::from_digraph`]'s rejecting default can name it.
    pub fn graph(&self) -> DiGraph {
        self.graph
    }
}

impl<V, E, K> AbstractDiGraphFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    /// Walk `graph`, binding `args` to its boundary ports on the first step.
    pub fn new(stage: CompileStage, index: EnvIndex, graph: DiGraph, args: Product<V>) -> Self {
        Self {
            stage,
            index,
            graph,
            pending: Some(args),
            schedule: None,
            yields: Vec::new(),
            resume_slots: None,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>
            + SparseForwardInterp<Frame = F>,
        F: AbstractFrameBuild<V, E, K>,
    {
        // First step: fetch the walk plan and bind the boundary ports.
        if let Some(args) = self.pending.take() {
            let plan = interp.digraph_walk_plan(self.stage, self.graph)?;
            if plan.ports.len() != args.len() {
                return Err(E::from(InterpreterError::ProductArityMismatch {
                    expected: plan.ports.len(),
                    actual: args.len(),
                }));
            }
            for (port, value) in plan.ports.iter().copied().zip(args) {
                interp.env_write(self.index, SSAValue::from(port), value)?;
            }
            self.schedule = Some(plan.schedule.into());
            self.yields = plan.yields;
            return Ok(FrameEffect::Continue(F::from_digraph(self)?));
        }

        let Some(statement) = self.schedule.as_mut().and_then(|s| s.pop_front()) else {
            return self.finish::<I, F>(interp);
        };

        match interp.run_statement(self.stage, statement, self.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(F::from_digraph(self)?)),
            SparseForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_digraph(self)?,
                    child: frame,
                })
            }
            // Summarize, don't descend: the interprocedural fixpoint
            // re-evaluates the callee under its own key.
            SparseForwardEffect::Call(call) => {
                let call_frame = AbstractCallFrame::new(self.stage, call, self.index);
                Ok(FrameEffect::Push {
                    parent: F::from_digraph(self)?,
                    child: F::from_call(call_frame),
                })
            }
            SparseForwardEffect::Jump(_) | SparseForwardEffect::Branch(_) => {
                Err(E::from(InterpreterError::CFGControlFlowInStructuredBody))
            }
            SparseForwardEffect::Yield(_) => Err(E::from(InterpreterError::Custom(
                "yield inside a digraph body (a digraph's outputs are its declared yields)",
            ))),
            SparseForwardEffect::Return(_) => Err(E::from(InterpreterError::Custom(
                "return inside a digraph body",
            ))),
        }
    }

    /// Schedule exhausted: read the declared yields out of the activation and
    /// complete. The parent decides what the values mean — a graph **owner**
    /// turns them into the function's return, a pushing statement binds them
    /// into its result slots.
    fn finish<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E>,
        F: AbstractFrameBuild<V, E, K>,
    {
        let values: Product<V> = self
            .yields
            .iter()
            .map(|&value| interp.env_read(self.index, value))
            .collect::<Result<_, _>>()?;
        Ok(FrameEffect::Complete(AbstractCompletion::Finished(Some(
            values,
        ))))
    }

    /// A pushed child finished without a payload (e.g. a summarized call whose
    /// results are already written): resume the schedule.
    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        F: AbstractFrameBuild<V, E, K>,
    {
        Ok(FrameEffect::Continue(F::from_digraph(self)?))
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
                        "digraph resume without result slots",
                    ))
                })?;
                interp.write_results(self.index, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_digraph(self)?))
            }
            // A nested push left via `return`. A digraph has no function-return
            // convention, so this cannot be relayed.
            AbstractCompletion::Finished(None) => Err(E::from(InterpreterError::Custom(
                "return bubbled into a digraph body",
            ))),
            AbstractCompletion::FunctionDone => Err(E::from(InterpreterError::Custom(
                "digraph frame resumed with a function completion",
            ))),
            AbstractCompletion::CFGBlock { .. } => Err(E::from(InterpreterError::Custom(
                "digraph frame resumed with a CFG-block completion",
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
    index: EnvIndex,
    _marker: PhantomData<fn() -> (E, K)>,
}

impl<V, E, K> AbstractCallFrame<V, E, K>
where
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    pub fn new(stage: CompileStage, call: CallEffect<V>, index: EnvIndex) -> Self {
        Self {
            stage,
            call,
            index,
            _marker: PhantomData,
        }
    }

    pub fn step_into<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E>
    where
        I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>,
        F: AbstractFrameBuild<V, E, K>,
    {
        interp.summarize_call(self.stage, self.call, self.index)?;
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
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
    DiGraph(AbstractDiGraphFrame<V, E, K>),
}

impl<V, E, K> AbstractFrameBuild<V, E, K> for StandardAbstractFrame<V, E, K> {
    fn from_block(frame: AbstractBlockFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Block(frame)
    }
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self {
        StandardAbstractFrame::Call(frame)
    }
    fn from_digraph(frame: AbstractDiGraphFrame<V, E, K>) -> Result<Self, E> {
        Ok(StandardAbstractFrame::DiGraph(frame))
    }
}

impl<I, V, E, K> Frame<I> for StandardAbstractFrame<V, E, K>
where
    I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>
        + SparseForwardInterp<Frame = StandardAbstractFrame<V, E, K>>,
    V: Clone + PartialEq,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    type Completion = AbstractCompletion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Block(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::Call(frame) => frame.step_into::<I, Self>(interp),
            StandardAbstractFrame::DiGraph(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardAbstractFrame::Call(frame) => frame.resume_done_into::<Self>(),
            StandardAbstractFrame::DiGraph(frame) => frame.resume_done_into::<Self>(),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardAbstractFrame::Block(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardAbstractFrame::Call(frame) => frame.resume_into::<Self>(completion),
            StandardAbstractFrame::DiGraph(frame) => {
                frame.resume_into::<I, Self>(completion, interp)
            }
        }
    }
}
