//! Shared frame protocol plus forward frame-driver capabilities.
//!
//! [`Frame`], [`FrameEngine`], [`FrameEffect`], and [`drive_frames`] are
//! direction-neutral. Forward engines add [`ForwardFrameDriver`] and
//! [`ForwardDataflowFrameDriver`] for env access, IR queries, calls, and abstract
//! merge/summarization.

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

/// Capabilities required by forward frames.
///
/// Re-exported as [`FrameDriver`](crate::FrameDriver).
pub trait ForwardFrameDriver: Env {
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
    /// Dispatch one statement to its dialect [`Interpretable`](crate::Interpretable)
    /// rule, producing this engine's [`Effect`](Interp::Effect) (a
    /// [`SparseForwardEffect`](crate::SparseForwardEffect) for the value engines).
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Result<Self::Effect, Self::Error>;
    /// Build the [`CallableBody`] a callable statement enters on invocation.
    fn enter_function(
        &mut self,
        stage: CompileStage,
        body: Statement,
        args: Product<Self::Value>,
        index: EnvIndex,
    ) -> Result<CallableBody<Self::Value>, Self::Error>;

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
    fn cfg_entry(&self, stage: CompileStage, cfg: CFG) -> Result<Option<Block>, Self::Error>;

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

    /// Bind a block's parameters to incoming actuals in `env` (arity-checked).
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

    /// Destructure `values` into `results` slots in `env` (arity-checked).
    fn write_results(
        &mut self,
        index: EnvIndex,
        results: &Product<SSAValue>,
        values: Product<Self::Value>,
    ) -> Result<(), Self::Error> {
        if results.len() != values.len() {
            return Err(Self::Error::from(InterpreterError::ProductArityMismatch {
                expected: results.len(),
                actual: values.len(),
            }));
        }
        for (slot, value) in results.iter().copied().zip(values) {
            self.env_write(index, slot, value)?;
        }
        Ok(())
    }
}

/// The **forward dataflow** frame-driver capability surface: what the forward
/// abstract frames need from the engine, beyond the [`ForwardFrameDriver`] IR
/// queries.
///
/// Implemented by [`SparseForwardInterpreter`](crate::SparseForwardInterpreter).
/// The standard abstract frames are generic over `I: ForwardDataflowFrameDriver`,
/// so a custom forward-dataflow frame can drive any engine providing these
/// capabilities.
///
/// The interprocedural protocol stays **atomic in the engine**: `summarize_call`
/// performs the whole call-summarization step (resolve, key, join arguments into
/// the callee entry summary, record the caller — *including same-key
/// self-recursion* — and read the current return summary or `bottom`), so a
/// custom frame cannot reorder it and break soundness. Frames only decide
/// *traversal*: which frame to step next.
///
/// Re-exported as [`AbstractFrameDriver`](crate::AbstractFrameDriver) for backward
/// compatibility.
pub trait ForwardDataflowFrameDriver: ForwardFrameDriver {
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
