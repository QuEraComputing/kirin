//! Total frame types for the toy language.
//!
//! The toy language uses `kirin-scf`, whose `scf.for` pushes a dialect-owned
//! loop frame ([`ScfForFrame`]/[`AbstractScfForFrame`]). A language that uses
//! such a dialect composes its own total frame enum embedding the standard
//! framework frames — the representation walkers
//! ([`BlockFrame`]/[`CFGFrame`]/[`DiGraphFrame`]) and the [`CallFrame`] call
//! boundary, via [`FrameBuild`]/[`AbstractFrameBuild`] — plus the dialect
//! frames (via [`BuildScfFor`]/[`BuildAbstractScfFor`]). The engine is
//! not forked — only the engine's `F` type parameter changes.

use std::hash::Hash;

use kirin_interpreter::engine::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractFrameBuild,
    AbstractFrameDriver, BlockFrame, CFGFrame, CallFrame, Completion, DiGraphFrame, Frame,
    FrameBuild, FrameDriver, FrameEffect, InterpreterError, SparseForwardInterp,
};
use kirin_scf::{
    AbstractScfForFrame, AbstractScfIfFrame, BuildAbstractScfFor, BuildAbstractScfIf, BuildScfFor,
    BuildScfIf, ForLoopValue, ScfForFrame, ScfIfFrame,
};

// ===========================================================================
// Concrete
// ===========================================================================

/// Concrete total frame: the standard representation walkers and call
/// boundary plus the SCF if/for frames.
pub enum ToyFrame<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
}

impl<V, E> FrameBuild<V, E> for ToyFrame<V, E> {
    fn from_block(frame: BlockFrame<V, E>) -> Self {
        ToyFrame::Block(frame)
    }
    fn from_cfg(frame: CFGFrame<V, E>) -> Self {
        ToyFrame::CFG(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        ToyFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self {
        ToyFrame::DiGraph(frame)
    }
}

impl<V, E> BuildScfIf<V, E> for ToyFrame<V, E> {
    fn scf_if(frame: ScfIfFrame<V, E>) -> Self {
        ToyFrame::ScfIf(frame)
    }
}

impl<V, E> BuildScfFor<V, E> for ToyFrame<V, E> {
    fn scf_for(frame: ScfForFrame<V, E>) -> Self {
        ToyFrame::ScfFor(frame)
    }
}

impl<I, F, V, E> Frame<I, F> for ToyFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = F>,
    F: FrameBuild<V, E> + BuildScfIf<V, E> + BuildScfFor<V, E>,
    V: Clone + ForLoopValue,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            ToyFrame::Block(frame) => frame.step_into(interp),
            ToyFrame::CFG(frame) => frame.step_into(interp),
            ToyFrame::Call(frame) => frame.step_into(interp),
            ToyFrame::DiGraph(frame) => frame.step_into(interp),
            ToyFrame::ScfIf(frame) => frame.step_into(interp),
            ToyFrame::ScfFor(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            ToyFrame::Block(frame) => frame.resume_done_into(interp),
            ToyFrame::CFG(frame) => frame.resume_done_into(interp),
            ToyFrame::Call(frame) => frame.resume_done_into(interp),
            ToyFrame::DiGraph(frame) => frame.resume_done_into(interp),
            ToyFrame::ScfIf(frame) => frame.resume_done_into(interp),
            ToyFrame::ScfFor(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            ToyFrame::Block(frame) => frame.resume_into(completion, interp),
            ToyFrame::CFG(frame) => frame.resume_into(completion, interp),
            ToyFrame::Call(frame) => frame.resume_into(completion, interp),
            ToyFrame::DiGraph(frame) => frame.resume_into(completion, interp),
            ToyFrame::ScfIf(frame) => frame.resume_into(completion, interp),
            ToyFrame::ScfFor(frame) => frame.resume_into(completion, interp),
        }
    }
}

// ===========================================================================
// Abstract
// ===========================================================================

/// Abstract total frame: standard abstract traversal plus the SCF if/for frames.
pub enum ToyAbstractFrame<V, E, K> {
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
    ScfIf(AbstractScfIfFrame<V, E, K>),
    ScfFor(AbstractScfForFrame<V, E, K>),
}

impl<V, E, K> AbstractFrameBuild<V, E, K> for ToyAbstractFrame<V, E, K> {
    fn from_block(frame: AbstractBlockFrame<V, E, K>) -> Self {
        ToyAbstractFrame::Block(frame)
    }
    fn from_call(frame: AbstractCallFrame<V, E, K>) -> Self {
        ToyAbstractFrame::Call(frame)
    }
}

impl<V, E, K> BuildAbstractScfIf<V, E, K> for ToyAbstractFrame<V, E, K> {
    fn scf_if(frame: AbstractScfIfFrame<V, E, K>) -> Self {
        ToyAbstractFrame::ScfIf(frame)
    }
}

impl<V, E, K> BuildAbstractScfFor<V, E, K> for ToyAbstractFrame<V, E, K> {
    fn scf_for(frame: AbstractScfForFrame<V, E, K>) -> Self {
        ToyAbstractFrame::ScfFor(frame)
    }
}

impl<I, F, V, E, K> Frame<I, F> for ToyAbstractFrame<V, E, K>
where
    I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K> + SparseForwardInterp<Frame = F>,
    F: AbstractFrameBuild<V, E, K> + BuildAbstractScfIf<V, E, K> + BuildAbstractScfFor<V, E, K>,
    V: Clone + PartialEq + ForLoopValue + Lattice,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    type Completion = AbstractCompletion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        match self {
            ToyAbstractFrame::Block(frame) => frame.step_into(interp),
            ToyAbstractFrame::Call(frame) => frame.step_into(interp),
            ToyAbstractFrame::ScfIf(frame) => frame.step_into(interp),
            ToyAbstractFrame::ScfFor(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(self, interp: &mut I) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        match self {
            ToyAbstractFrame::Block(frame) => frame.resume_done_into(interp),
            ToyAbstractFrame::Call(frame) => frame.resume_done_into(interp),
            ToyAbstractFrame::ScfIf(frame) => frame.resume_done_into(interp),
            ToyAbstractFrame::ScfFor(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: AbstractCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, AbstractCompletion<V>>, E> {
        match self {
            ToyAbstractFrame::Block(frame) => frame.resume_into(completion, interp),
            ToyAbstractFrame::Call(frame) => frame.resume_into(completion, interp),
            ToyAbstractFrame::ScfIf(frame) => frame.resume_into(completion, interp),
            ToyAbstractFrame::ScfFor(frame) => frame.resume_into(completion, interp),
        }
    }
}

// ===========================================================================
// Dense backward (classic per-point liveness)
// ===========================================================================

use kirin_interpreter::DenseBackwardState;
use kirin_interpreter::engine::{
    DenseBackwardCompletion, DenseBackwardFrameDriver, DenseBlockFrame, DenseFrameBuild,
};
use kirin_scf::{BuildDenseScfFor, BuildDenseScfIf, DenseScfForFrame, DenseScfIfFrame};

use kirin::prelude::Lattice;

/// Dense backward total frame: the standard block walk plus the SCF dense
/// frames (arm-join for `scf.if`, the loop-carried fixpoint for `scf.for`).
pub enum ToyDenseBackwardFrame<V, E> {
    Block(DenseBlockFrame<V, E>),
    ScfIf(DenseScfIfFrame<V, E>),
    ScfFor(DenseScfForFrame<V, E>),
}

impl<V, E> DenseFrameBuild<V, E> for ToyDenseBackwardFrame<V, E> {
    fn from_block(frame: DenseBlockFrame<V, E>) -> Self {
        ToyDenseBackwardFrame::Block(frame)
    }
}

impl<V, E> BuildDenseScfIf<V, E> for ToyDenseBackwardFrame<V, E> {
    fn scf_if(frame: DenseScfIfFrame<V, E>) -> Self {
        ToyDenseBackwardFrame::ScfIf(frame)
    }
}

impl<V, E> BuildDenseScfFor<V, E> for ToyDenseBackwardFrame<V, E> {
    fn scf_for(frame: DenseScfForFrame<V, E>) -> Self {
        ToyDenseBackwardFrame::ScfFor(frame)
    }
}

impl<I, F, V, E> Frame<I, F> for ToyDenseBackwardFrame<V, E>
where
    I: DenseBackwardFrameDriver<Value = V, Error = E, Frame = F>,
    F: DenseFrameBuild<V, E> + BuildDenseScfIf<V, E> + BuildDenseScfFor<V, E>,
    V: Clone + PartialEq + Lattice + DenseBackwardState,
    E: From<InterpreterError>,
{
    type Completion = DenseBackwardCompletion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match self {
            ToyDenseBackwardFrame::Block(frame) => frame.step_into(interp),
            ToyDenseBackwardFrame::ScfIf(frame) => frame.step_into(interp),
            ToyDenseBackwardFrame::ScfFor(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match self {
            ToyDenseBackwardFrame::Block(frame) => frame.resume_done_into(interp),
            ToyDenseBackwardFrame::ScfIf(frame) => frame.resume_done_into(interp),
            ToyDenseBackwardFrame::ScfFor(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: DenseBackwardCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match self {
            ToyDenseBackwardFrame::Block(frame) => frame.resume_into(completion, interp),
            ToyDenseBackwardFrame::ScfIf(frame) => frame.resume_into(completion, interp),
            ToyDenseBackwardFrame::ScfFor(frame) => frame.resume_into(completion, interp),
        }
    }
}
