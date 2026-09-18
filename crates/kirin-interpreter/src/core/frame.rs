//! Shared frame protocol plus the engine capabilities frames require.
//!
//! [`Frame`], [`FrameEngine`], [`FrameEffect`], and [`drive_frames`] are
//! direction-neutral. Forward engines add the capability traits below.
//!
//! # Three levels of "engine"
//!
//! The word means something different at each level, so the names are kept
//! distinct:
//!
//! - **[`drive_frames`]** is the *frame-stack driver* — the loop. Nothing else
//!   is a "driver"; the concrete objects named `ForwardDriver` /
//!   `DenseBackwardDriver` are fixpoint-driver structs, not capability traits.
//! - **[`FrameEngine`]** is the minimal engine contract the generic frame stack
//!   needs: a total `Error` type, and nothing more.
//! - the **component traits** below are narrowly scoped services an interpreter
//!   engine supplies *to individual frames*, and the two **umbrellas**
//!   ([`ForwardFrameEngine`], [`ForwardDataflowFrameEngine`]) name the full
//!   capability set for a whole standard frame universe.
//!
//! # The capability model
//!
//! Capabilities are split by **what one frame needs**, not by what one engine
//! happens to provide. Each trait is the requirement of a specific kind of
//! traversal, so a frame's bound documents exactly which engine operations it
//! can reach — and an engine that implements only some of them still runs the
//! frames it can support.
//!
//! | trait | capability | consumed by |
//! |---|---|---|
//! | [`StatementDispatch`] | dispatch a statement to its dialect rule | every executing frame |
//! | [`BlockQueries`] | read-only structural queries for walking one block | [`BlockFrame`](crate::BlockFrame), [`AbstractBlockFrame`](crate::AbstractBlockFrame), dialect block walkers |
//! | [`CFGQueries`] | find a CFG's entry block (`: BlockQueries`) | [`CFGFrame`](crate::CFGFrame) |
//! | [`DiGraphQueries`] | schedule a digraph body | [`DiGraphFrame`](crate::DiGraphFrame) |
//! | [`CallServices`] | activation storage, linking, callable-entry dispatch | [`CallFrame`](crate::CallFrame) |
//!
//! The `*Queries` traits are exactly that: **read-only**. The one operation that
//! needs both a query and a write — binding a block's parameters to incoming
//! actuals — lives on the crate-private `BlockBinding` extension instead of
//! hiding inside [`BlockQueries`], so no query trait's name conceals a store
//! mutation.
//!
//! [`StatementDispatch`] is the engine side of dialect dispatch, and is easy to
//! confuse with [`InterpDispatch`](crate::InterpDispatch) — they face opposite
//! directions. `InterpDispatch<I>` is implemented by a **stage/language** to
//! route a statement to the right dialect rule. `StatementDispatch` is
//! implemented by the **engine** and is what a *frame* calls: it stashes the
//! current location (`stage`/`statement`/`index`) so the rule can read it back
//! through [`Interp`], then delegates to `InterpDispatch`.
//!
//! Two umbrellas compose the components for the two *engine families*. A total
//! frame enum belongs on an umbrella — a universe's engine must support the
//! union of all its variants — while a member frame names only its components:
//!
//! - [`ForwardFrameEngine`] — the full concrete surface: all four components,
//!   blanket-implemented.
//! - [`ForwardDataflowFrameEngine`] — abstract dataflow: the traversal
//!   components abstract execution *shares*, plus merge/summarization. It does
//!   **not** inherit [`CallServices`] or [`CFGQueries`], because an abstract
//!   engine summarizes calls rather than entering them.

use std::hash::Hash;

use kirin_ir::{Block, CFG, CompileStage, Product, SSAValue, Statement};

use crate::{
    Body, CallEffect, CallableBody, Callee, Env, EnvIndex, FunctionTarget, Interp, InterpreterError,
};

/// Structural effect a [`Frame`] returns to the engine driver loop.
pub enum FrameEffect<F, C> {
    /// Replace the top of the stack with `F` and keep running.
    Continue(F),
    /// Push `parent` then `child`; `child` runs next, `parent` resumes after.
    Push { parent: F, child: F },
    /// This frame finished with no payload; its parent's
    /// [`Frame::resume_done_into`] is called.
    Done,
    /// This frame produced a completion `C`; its parent's
    /// [`Frame::resume_into`] is called (or, at the root, the run finishes with
    /// `C`).
    Complete(C),
}

/// Minimal capability a [`Frame`] stack needs from the engine driving it.
pub trait FrameEngine {
    /// The total error type produced while stepping frames.
    type Error;
}

impl<T: Interp> FrameEngine for T {
    type Error = <T as Interp>::Error;
}

/// A continuation frame anchored in an IR traversal, expressed over the total
/// frame type `F` it composes into.
///
/// Every method consumes `self` and returns the next structural move as a
/// [`FrameEffect`] **over `F`** — never over `Self`. That single choice is what
/// lets one trait serve both roles a frame stack needs:
///
/// - a **member** — an individual walker ([`BlockFrame`](crate::BlockFrame),
///   [`CallFrame`](crate::CallFrame), a dialect's own frame). It is one variant
///   of `F` and names its successors in `F`, re-wrapping itself through the
///   relevant `*FrameBuild` hook. Members are generic over `F`, so the same
///   walker composes into any language's frame type.
/// - a **universe** — a language's total frame enum. It implements
///   `Frame<I, Self>` when it is the stack's element type, and stays generic
///   over `F` so that it can *also* be embedded in a larger enum (an
///   instrumenting wrapper, or another language's frame type) without
///   re-enumerating its variants.
///
/// [`drive_frames`] bounds on `F: Frame<I, F>`: the stack's element type must be
/// a universe — a type able to represent every frame that can appear on it.
pub trait Frame<I: FrameEngine, F>: Sized {
    /// The completion payload this frame family bubbles to parents/root.
    type Completion;

    /// Do this frame's next unit of work.
    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, Self::Completion>, I::Error>;

    /// A pushed child finished with no payload.
    fn resume_done_into(self, interp: &mut I)
    -> Result<FrameEffect<F, Self::Completion>, I::Error>;

    /// A pushed child finished with a completion payload.
    fn resume_into(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Self::Completion>, I::Error>;
}

/// Shared frame-stepping loop.
pub fn drive_frames<I, F>(engine: &mut I, frames: &mut Vec<F>) -> Result<F::Completion, I::Error>
where
    I: FrameEngine,
    I::Error: From<InterpreterError>,
    F: Frame<I, F>,
{
    loop {
        let frame = frames
            .pop()
            .ok_or_else(|| I::Error::from(InterpreterError::EmptyFrameStack))?;
        let mut effect = frame.step_into(engine)?;
        loop {
            match effect {
                FrameEffect::Continue(frame) => {
                    frames.push(frame);
                    break;
                }
                FrameEffect::Push { parent, child } => {
                    frames.push(parent);
                    frames.push(child);
                    break;
                }
                FrameEffect::Done => {
                    let parent = frames
                        .pop()
                        .ok_or_else(|| I::Error::from(InterpreterError::EmptyFrameStack))?;
                    effect = parent.resume_done_into(engine)?;
                }
                FrameEffect::Complete(completion) => match frames.pop() {
                    Some(parent) => {
                        effect = parent.resume_into(completion, engine)?;
                    }
                    None => return Ok(completion),
                },
            }
        }
    }
}

/// Engine capability for dispatching a statement to its dialect rule.
///
/// The one capability *every* frame that executes statements needs, and the only
/// one shared by concrete execution and abstract dataflow.
///
/// Not to be confused with [`InterpDispatch`](crate::InterpDispatch), which
/// faces the other way: a *stage/language* implements `InterpDispatch` to route
/// a statement to its dialect rule, while an *engine* implements
/// `StatementDispatch` to expose location-aware dispatch to frames.
pub trait StatementDispatch: Interp {
    /// Dispatch one statement to its dialect [`Interpretable`](crate::Interpretable)
    /// rule, producing this engine's [`Effect`](Interp::Effect) (a
    /// [`SparseForwardEffect`](crate::SparseForwardEffect) for the value engines).
    ///
    /// The engine stashes `stage`/`statement`/`index` as its current location
    /// first, so the rule can read it back through [`Interp`].
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<Self::Effect, Self::Error>;
}

/// Read-only structural queries needed to traverse a single [`Block`].
///
/// The requirement of [`BlockFrame`](crate::BlockFrame), its internal
/// `BlockCursor`, and every frame that steps through a block's statements.
///
/// **Read-only by construction**: only [`Interp`] is required, not [`Env`], so
/// nothing on this trait can touch SSA storage. Entering a block also *binds*
/// its parameters, which needs a write — that operation lives on the
/// crate-private `BlockBinding` extension (bounded `Env + BlockQueries`)
/// rather than here, so this name cannot hide a store mutation. Engine-internal
/// callers wanting the same queries outside a frame use
/// [`StageQuery`](crate::StageQuery).
pub trait BlockQueries: Interp {
    fn block_params(&self, stage: CompileStage, block: Block)
    -> Result<Vec<SSAValue>, Self::Error>;
    fn first_statement(
        &self,
        stage: CompileStage,
        block: Block,
    ) -> Result<Option<Statement>, Self::Error>;
    fn next_statement(
        &self,
        stage: CompileStage,
        block: Block,
        after: Statement,
    ) -> Result<Option<Statement>, Self::Error>;
}

/// Structural queries needed to enter and traverse a [`CFG`].
///
/// Extends [`BlockQueries`] because walking a CFG *is* walking its blocks and
/// following jumps between them; `cfg_entry` only adds finding where to start.
pub trait CFGQueries: BlockQueries {
    fn cfg_entry(&self, stage: CompileStage, cfg: CFG) -> Result<Option<Block>, Self::Error>;
}

/// Crate-private block-entry binding: the one operation that needs a
/// [`BlockQueries`] read *and* an [`Env`] write.
///
/// Deliberately not on [`BlockQueries`] (whose name promises read-only) and
/// deliberately not public: it is frame-internal mechanics, blanket-implemented
/// for every engine with both capabilities, so a frame that binds a block entry
/// spells its requirement honestly as `Env + BlockQueries`.
pub(crate) trait BlockBinding: Env + BlockQueries {
    /// Positionally bind a block's parameters to incoming actuals in `index`,
    /// checking arity.
    fn bind_block_args(
        &mut self,
        stage: CompileStage,
        index: EnvIndex,
        block: Block,
        args: &Product<Self::Value>,
    ) -> Result<(), Self::Error> {
        let params = self.block_params(stage, block)?;
        if params.len() != args.len() {
            return Err(Self::Error::from(InterpreterError::BlockArityMismatch {
                block,
                expected: params.len(),
                actual: args.len(),
            }));
        }
        for (param, value) in params.into_iter().zip(args.iter().cloned()) {
            self.env_write(index, param, value)?;
        }
        Ok(())
    }
}

impl<T: Env + BlockQueries> BlockBinding for T {}

/// Structural/scheduling queries needed to traverse a
/// [`DiGraph`](kirin_ir::DiGraph) body.
///
/// Split out from the block/CFG queries because a digraph walk shares none of
/// their mechanics: there are no blocks, no jumps, and no entry block — only a
/// dependency order.
pub trait DiGraphQueries: Interp {
    /// The default walk plan of a digraph body (ports, toposorted nodes,
    /// yields). Errors on cyclic digraphs.
    ///
    /// Digraph bodies are opt-in: an engine that never walks one inherits this
    /// rejection rather than inventing a schedule, the same way
    /// [`FrameBuild::from_ungraph_entry`](crate::FrameBuild::from_ungraph_entry)
    /// rejects a callable `UnGraph` without a compiler-supplied policy.
    fn digraph_walk_plan(
        &self,
        stage: CompileStage,
        graph: kirin_ir::DiGraph,
    ) -> Result<crate::GraphWalkPlan, Self::Error> {
        let _ = stage;
        Err(Self::Error::from(InterpreterError::NoDefaultWalker(
            Body::DiGraph(graph),
        )))
    }
}

/// Engine services used by [`CallFrame`](crate::CallFrame): activation storage,
/// linking, and callable-entry dispatch.
///
/// **[`CallFrame`](crate::CallFrame) still owns the calling convention** — the
/// order of operations, which completions are legal, and freeing the activation
/// exactly once. This trait only supplies the primitives it calls.
///
/// Kept whole on purpose: the standard `CallFrame` consumes all four together,
/// and their pairing is a safety property — an `alloc_env` without its matching
/// `free_env` is a leak, a second `free_env` a double free. Splitting them into
/// separate capabilities would let an engine offer half a call convention.
///
/// Notably *not* required by abstract dataflow: forward abstract interpretation
/// summarizes a call instead of descending into it, so
/// [`ForwardDataflowFrameEngine`] does not extend this trait.
pub trait CallServices: Env {
    /// Allocate a fresh SSA activation record.
    fn alloc_env(&mut self) -> EnvIndex;
    /// Free an activation record.
    fn free_env(&mut self, index: EnvIndex) -> Result<(), Self::Error>;
    /// Resolve a callee to a concrete function target via the engine's linker.
    fn resolve_call(
        &self,
        stage: CompileStage,
        callee: &Callee,
    ) -> Result<FunctionTarget, Self::Error>;
    /// Build the [`CallableBody`] a callable statement enters on invocation.
    fn enter_function(
        &mut self,
        stage: CompileStage,
        body: Statement,
        args: Product<Self::Value>,
        index: EnvIndex,
    ) -> Result<CallableBody<Self::Value>, Self::Error>;
}

/// An interpreter engine capable of running the complete standard **concrete**
/// forward-frame universe.
///
/// This is an umbrella, not a definition — it adds no methods and is
/// [blanket-implemented](#impl-ForwardFrameEngine-for-T) for any engine
/// providing the four components. Use it at the *universe* level, where a total
/// frame enum's engine must support the union of all its variants
/// ([`StandardFrame`](crate::StandardFrame) and downstream frame enums do).
/// Individual member frames should bound only the components they use, so a
/// partial engine can still run them.
pub trait ForwardFrameEngine:
    StatementDispatch + CFGQueries + DiGraphQueries + CallServices
{
}

impl<T> ForwardFrameEngine for T where
    T: StatementDispatch + CFGQueries + DiGraphQueries + CallServices
{
}

/// An interpreter engine capable of running the standard **forward abstract**
/// frame universe: the traversal capabilities it shares with concrete execution,
/// plus merge/summarization.
///
/// It extends [`Env`] + [`StatementDispatch`] + [`BlockQueries`] +
/// [`DiGraphQueries`] — the traversal it genuinely shares — and **deliberately
/// not** [`CallServices`] or [`CFGQueries`]. An abstract engine does not descend
/// into a callee (it [summarizes](Self::summarize_call) the call), so requiring
/// it to expose concrete activation allocation, activation cleanup,
/// `resolve_call`, and `enter_function` would be demanding a call convention it
/// never performs. `cfg_entry` is likewise absent: the forward abstract engine
/// reaches a callable body's entry block through [`Owner`](crate::Owner) seeding
/// in the fixpoint driver, not by asking a frame to enter a CFG. A frame that
/// *does* want either capability can name it in addition — see the
/// abstract-body-traversal follow-up.
///
/// Implemented by [`SparseForwardInterpreter`](crate::SparseForwardInterpreter).
/// The standard abstract frames are generic over
/// `I: ForwardDataflowFrameEngine`, so a custom forward-dataflow frame can drive
/// any engine providing these capabilities.
///
/// The interprocedural protocol stays **atomic in the engine**: `summarize_call`
/// performs the whole call-summarization step (resolve, key, join arguments into
/// the callee entry summary, record the caller — *including same-key
/// self-recursion* — and read the current return summary or `bottom`), so a
/// custom frame cannot reorder it and break soundness. Frames only decide
/// *traversal*: which frame to step next.
pub trait ForwardDataflowFrameEngine:
    Env + StatementDispatch + BlockQueries + DiGraphQueries
{
    /// The key under which function entry/return summaries are tracked
    /// (the analysis [`CallContext::Key`](crate::CallContext::Key)).
    type SummaryKey: Clone + Eq + Hash;

    /// Combine `incoming` into `current` at a merge point via the analysis
    /// [`WideningStrategy`](crate::WideningStrategy) (join vs. widen by `visits`).
    fn analysis_merge(
        &self,
        current: &Product<Self::Value>,
        incoming: &Product<Self::Value>,
        visits: usize,
    ) -> Result<Product<Self::Value>, Self::Error>;

    /// Fold a `Return` product into the function-evaluation return accumulator.
    fn contribute_return(&mut self, values: Product<Self::Value>) -> Result<(), Self::Error>;

    /// The summary key of the function currently being evaluated (the caller, for
    /// recording call dependencies — including same-key recursion).
    fn current_function_key(&self) -> Option<Self::SummaryKey>;

    /// Summarize a call atomically (the engine's interprocedural protocol):
    /// resolve, key, join arguments into the callee entry, record the caller
    /// (including self), and write the callee's current return summary (or
    /// per-slot `bottom` until it converges) into `env`.
    fn summarize_call(
        &mut self,
        stage: CompileStage,
        call: CallEffect<Self::Value>,
        index: EnvIndex,
    ) -> Result<(), Self::Error>;

    /// The per-fixpoint iteration cap (divergence guard).
    fn max_iterations(&self) -> usize;
}
