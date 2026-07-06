//! Total frame types for the toy language.
//!
//! The toy language uses `kirin-scf`, whose `scf.for` pushes a dialect-owned
//! loop frame ([`ScfForFrame`]/[`AbstractScfForFrame`]). A language that uses
//! such a dialect composes its own total frame enum embedding the standard
//! framework frames (via [`FrameBuild`]/[`AbstractFrameBuild`]) plus the
//! dialect frames (via [`BuildScfFor`]/[`BuildAbstractScfFor`]). The engine is
//! not forked — only the engine's `F` type parameter changes.

use std::hash::Hash;

use kirin_interpreter::engine::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractFrameBuild,
    AbstractFrameDriver, BodyFrame, CallFrame, Completion, Frame, FrameBuild, FrameDriver,
    FrameEffect, InterpreterError, SparseForwardInterp,
};
use kirin_scf::{
    AbstractScfForFrame, AbstractScfIfFrame, BuildAbstractScfFor, BuildAbstractScfIf, BuildScfFor,
    BuildScfIf, ForLoopValue, ScfForFrame, ScfIfFrame,
};

// ===========================================================================
// Concrete
// ===========================================================================

/// Concrete total frame: standard body/call traversal plus the SCF if/for frames.
pub enum ToyFrame<V, E> {
    Body(BodyFrame<V, E>),
    Call(CallFrame<V>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
}

impl<V, E> FrameBuild<V, E> for ToyFrame<V, E> {
    fn from_body(frame: BodyFrame<V, E>) -> Self {
        ToyFrame::Body(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        ToyFrame::Call(frame)
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

impl<I, V, E> Frame<I> for ToyFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = ToyFrame<V, E>>,
    V: Clone + ForLoopValue,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyFrame::Body(frame) => frame.step_into::<I, Self>(interp),
            ToyFrame::Call(frame) => frame.step_into::<I, Self>(interp),
            ToyFrame::ScfIf(frame) => frame.step_into::<I, Self>(interp),
            ToyFrame::ScfFor(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyFrame::Body(frame) => Ok(frame.resume_done_into::<Self>()),
            ToyFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(I::Error::from),
            ToyFrame::ScfIf(frame) => frame.resume_done_into::<Self>(),
            ToyFrame::ScfFor(frame) => frame.resume_done_into::<Self>(),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyFrame::Body(frame) => frame.resume_into::<I, Self>(completion, interp),
            ToyFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
            ToyFrame::ScfIf(frame) => frame.resume_into::<Self>(completion),
            ToyFrame::ScfFor(frame) => frame.resume_into::<I, Self>(completion, interp),
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

impl<I, V, E, K> Frame<I> for ToyAbstractFrame<V, E, K>
where
    I: AbstractFrameDriver<Value = V, Error = E, SummaryKey = K>
        + SparseForwardInterp<Frame = ToyAbstractFrame<V, E, K>>,
    V: Clone + PartialEq + ForLoopValue,
    E: From<InterpreterError>,
    K: Clone + Eq + Hash,
{
    type Completion = AbstractCompletion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyAbstractFrame::Block(frame) => frame.step_into::<I, Self>(interp),
            ToyAbstractFrame::Call(frame) => frame.step_into::<I, Self>(interp),
            ToyAbstractFrame::ScfIf(frame) => frame.step_into::<I, Self>(interp),
            ToyAbstractFrame::ScfFor(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyAbstractFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
            ToyAbstractFrame::Call(frame) => frame.resume_done_into::<Self>(),
            ToyAbstractFrame::ScfIf(frame) => frame.resume_done_into::<Self>(),
            ToyAbstractFrame::ScfFor(frame) => frame.resume_done_into::<Self>(),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyAbstractFrame::Block(frame) => frame.resume_into::<I, Self>(completion, interp),
            ToyAbstractFrame::Call(frame) => frame.resume_into::<Self>(completion),
            ToyAbstractFrame::ScfIf(frame) => frame.resume_into::<I, Self>(completion, interp),
            ToyAbstractFrame::ScfFor(frame) => frame.resume_into::<I, Self>(completion, interp),
        }
    }
}

// ===========================================================================
// Dense backward (classic per-point liveness)
// ===========================================================================

use kirin_interpreter::PointFacts;
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

impl<I, V, E> Frame<I> for ToyDenseBackwardFrame<V, E>
where
    I: DenseBackwardFrameDriver<Value = V, Error = E, Frame = ToyDenseBackwardFrame<V, E>>,
    V: Clone + PartialEq + Lattice + PointFacts,
    E: From<InterpreterError>,
{
    type Completion = DenseBackwardCompletion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyDenseBackwardFrame::Block(frame) => frame.step_into::<I, Self>(interp),
            ToyDenseBackwardFrame::ScfIf(frame) => frame.step_into::<I, Self>(interp),
            ToyDenseBackwardFrame::ScfFor(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyDenseBackwardFrame::Block(frame) => frame.resume_done_into::<Self>(),
            ToyDenseBackwardFrame::ScfIf(frame) => frame.resume_done_into::<Self>(),
            ToyDenseBackwardFrame::ScfFor(frame) => frame.resume_done_into::<Self>(),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            ToyDenseBackwardFrame::Block(frame) => frame.resume_into::<I, Self>(completion, interp),
            ToyDenseBackwardFrame::ScfIf(frame) => frame.resume_into::<I, Self>(completion, interp),
            ToyDenseBackwardFrame::ScfFor(frame) => {
                frame.resume_into::<I, Self>(completion, interp)
            }
        }
    }
}
