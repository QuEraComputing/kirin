//! The shared frame-based traversal **protocol**.
//!
//! The dialect API ([`Interpretable`](crate::Interpretable)) produces a closed
//! [`ForwardEffect`] per statement. This module is the layer *between* that dialect
//! algebra and the engines: a **frame** consumes effects and decides traversal,
//! and an engine just runs a stack of frames. This module owns only the
//! protocol; the two implementations of it live alongside:
//!
//! - [`Frame`] is the continuation trait. The *total* frame type `F` (an enum
//!   of frame kinds) implements it; the engine owns a `Vec<F>` and applies the
//!   returned [`FrameEffect`]. Frame generics never appear in
//!   [`Interpretable`](crate::Interpretable).
//! - [`FrameDriver`] is the capability surface every frame needs from its engine
//!   (env alloc/free, IR queries, statement dispatch, call resolution). Both
//!   engines implement it.
//! - [`AbstractFrameDriver`] adds the abstract-only capabilities (analysis
//!   merge, return accumulation, atomic call summarization).
//! - The **concrete** implementation — [`ScopeFrame`](crate::ScopeFrame),
//!   [`CallFrame`](crate::CallFrame), [`StandardFrame`](crate::StandardFrame) —
//!   lives in [`concrete_frame`](crate::concrete_frame); the **abstract**
//!   implementation lives in [`abstract_frame`](crate::abstract_frame). Both are
//!   implementations of *this* protocol, not parallel frameworks.

use std::hash::Hash;

use kirin_ir::{Block, CompileStage, Product, Region, SSAValue, Statement};

use crate::{
    CallEffect, Callee, EnvIndex, ForwardEffect, FunctionTarget, Interp, InterpreterError, Scope,
};

/// Structural effect a [`Frame`] returns to the engine driver loop.
pub enum FrameEffect<F, C> {
    /// Replace the top of the stack with `F` and keep running.
    Continue(F),
    /// Push `parent` then `child`; `child` runs next, `parent` resumes after.
    Push { parent: F, child: F },
    /// This frame finished with no payload; its parent's
    /// [`Frame::resume_done`] is called.
    Done,
    /// This frame produced a completion `C`; its parent's [`Frame::resume`] is
    /// called (or, at the root, the run finishes with `C`).
    Complete(C),
}

/// The minimal capability a [`Frame`] stack needs from the engine driving it:
/// just a total error type. This is the *direction-neutral* anchor of the frame
/// protocol — deliberately saying nothing about a value domain — so a frame
/// family is decoupled from the forward value engine ([`Interp`]). Every
/// [`Interp`] is a `FrameEngine` (blanket impl below); a future engine that is
/// not an `Interp` can implement this directly.
pub trait FrameEngine {
    /// The total error type produced while stepping frames.
    type Error;
}

impl<T: Interp> FrameEngine for T {
    type Error = <T as Interp>::Error;
}

/// A continuation frame anchored in an IR traversal. Implemented by the *total*
/// frame type `F`; each method consumes `self` and returns the next structural
/// move as a [`FrameEffect`].
///
/// `I` is the engine, constrained only by [`FrameEngine`] here (a total error
/// type) — frames do not require the forward [`Interp`] value engine. Standard
/// frames additionally require [`FrameDriver`]/[`AbstractFrameDriver`] in their
/// own impls, so the trait itself does not leak engine capabilities.
pub trait Frame<I: FrameEngine>: Sized {
    /// The completion payload this frame family bubbles to parents/root.
    type Completion;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
}

/// The shared frame-stepping driver loop, used by every engine instead of each
/// re-implementing the same worklist: pop the top frame, [`step`](Frame::step)
/// it, and apply the returned [`FrameEffect`]. `Continue`/`Push` adjust the
/// stack; `Done`/`Complete` bubble through parents via
/// [`resume_done`](Frame::resume_done)/[`resume`](Frame::resume) until one
/// continues or the stack empties. A `Complete` at the root returns its
/// completion to the caller.
pub fn drive_frames<I, F>(engine: &mut I, frames: &mut Vec<F>) -> Result<F::Completion, I::Error>
where
    I: FrameEngine,
    I::Error: From<InterpreterError>,
    F: Frame<I>,
{
    loop {
        let frame = frames
            .pop()
            .ok_or_else(|| I::Error::from(InterpreterError::EmptyFrameStack))?;
        let mut effect = frame.step(engine)?;
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
                    effect = parent.resume_done(engine)?;
                }
                FrameEffect::Complete(completion) => match frames.pop() {
                    Some(parent) => {
                        effect = parent.resume(completion, engine)?;
                    }
                    None => return Ok(completion),
                },
            }
        }
    }
}

/// Capabilities a frame needs from its engine, beyond [`Interp`]'s env access.
///
/// Implemented by both engines ([`ConcreteInterpreter`](crate::ConcreteInterpreter)
/// and [`AbstractInterpreter`](crate::AbstractInterpreter)), so the standard
/// frames are generic over `I: FrameDriver` and a custom frame can drive any
/// engine providing these capabilities.
pub trait FrameDriver: Interp {
    /// Allocate a fresh SSA activation record.
    fn alloc_env(&mut self) -> EnvIndex;
    /// Free an activation record.
    fn free_env(&mut self, env: EnvIndex) -> Result<(), Self::Error>;
    /// Resolve a callee to a concrete function target via the engine's linker.
    fn resolve_call(
        &self,
        stage: CompileStage,
        callee: &Callee,
    ) -> Result<FunctionTarget, Self::Error>;
    /// Dispatch one statement to its dialect [`Interpretable`](crate::Interpretable)
    /// rule, producing an [`ForwardEffect`].
    fn run_statement(
        &mut self,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
    ) -> Result<ForwardEffect<Self::Value, Self::Error>, Self::Error>;
    /// Build the entry [`Scope`] a callable statement enters on invocation.
    fn enter_function(
        &mut self,
        stage: CompileStage,
        body: Statement,
        args: Product<Self::Value>,
        env: EnvIndex,
    ) -> Result<Scope<Self::Value, Self::Error>, Self::Error>;

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
    fn region_entry(
        &self,
        stage: CompileStage,
        region: Region,
    ) -> Result<Option<Block>, Self::Error>;

    /// Bind a block's parameters to incoming actuals in `env` (arity-checked).
    fn bind_block_args(
        &mut self,
        stage: CompileStage,
        env: EnvIndex,
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
            self.env_write(env, param, value)?;
        }
        Ok(())
    }

    /// Destructure `values` into `results` slots in `env` (arity-checked).
    fn write_results(
        &mut self,
        env: EnvIndex,
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
            self.env_write(env, slot, value)?;
        }
        Ok(())
    }
}

/// Capabilities the **abstract** frames need from the abstract engine, beyond
/// the shared [`FrameDriver`] IR queries.
///
/// Implemented by [`AbstractInterpreter`](crate::AbstractInterpreter). The
/// standard abstract frames are generic over `I: AbstractFrameDriver`, so a
/// custom abstract frame can drive any engine providing these capabilities.
///
/// The interprocedural protocol stays **atomic in the engine**: `summarize_call`
/// performs the whole call-summarization step (resolve, key, join arguments into
/// the callee entry summary, record the caller — *including same-key
/// self-recursion* — and read the current return summary or `bottom`), so a
/// custom frame cannot reorder it and break soundness. Frames only decide
/// *traversal*: which frame to step next.
pub trait AbstractFrameDriver: FrameDriver {
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
        env: EnvIndex,
    ) -> Result<(), Self::Error>;

    /// The per-fixpoint iteration cap (divergence guard).
    fn max_iterations(&self) -> usize;
}
