//! Total frame types for the toy language.
//!
//! The toy language uses `kirin-scf`, whose `scf.for` pushes a dialect-owned
//! loop frame ([`ScfForFrame`]/[`AbstractScfForFrame`]). A language that uses
//! such a dialect explicitly composes the standard framework frames — the representation walkers
//! ([`BlockFrame`]/[`CFGFrame`]/[`DiGraphFrame`]) and the [`CallFrame`] call
//! boundary — plus the dialect frames. The abstract engines retain
//! [`AbstractFrameBuild`] and [`BuildAbstractScfFor`] pending separate review.

use std::hash::Hash;

use kirin_interpreter::engine::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractFrameBuild, BlockFrame,
    CFGFrame, CallFrame, CallRequest, Completion, DiGraphFrame, ForwardDataflowFrameEngine, Frame,
    FrameEffect, FrameEngine, InterpreterError, SparseForwardInterp,
};
use kirin_scf::{
    AbstractScfForFrame, AbstractScfIfFrame, BuildAbstractScfFor, BuildAbstractScfIf, ForLoopValue,
    ScfForFrame, ScfIfFrame,
};

// ===========================================================================
// Concrete
// ===========================================================================

/// The toy language's private concrete continuation stack element.
///
/// This enum is the language's explicit composition root: it records which
/// framework and dialect-owned computations may coexist on one stack. Member
/// frames never name it or construct its variants.
pub(crate) enum FrameStackItem<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
}

impl<V, E> From<BlockFrame<V, E>> for FrameStackItem<V, E> {
    fn from(frame: BlockFrame<V, E>) -> Self {
        Self::Block(frame)
    }
}

impl<V, E> From<CFGFrame<V, E>> for FrameStackItem<V, E> {
    fn from(frame: CFGFrame<V, E>) -> Self {
        Self::CFG(frame)
    }
}

impl<V, E> From<CallRequest<V>> for FrameStackItem<V, E> {
    fn from(request: CallRequest<V>) -> Self {
        Self::Call(request.into())
    }
}

impl<V, E> From<DiGraphFrame<V, E>> for FrameStackItem<V, E> {
    fn from(frame: DiGraphFrame<V, E>) -> Self {
        Self::DiGraph(frame)
    }
}

impl<V, E> From<ScfIfFrame<V, E>> for FrameStackItem<V, E> {
    fn from(frame: ScfIfFrame<V, E>) -> Self {
        Self::ScfIf(frame)
    }
}

impl<V, E> From<ScfForFrame<V, E>> for FrameStackItem<V, E> {
    fn from(frame: ScfForFrame<V, E>) -> Self {
        Self::ScfFor(frame)
    }
}

impl<I, V, E> Frame<I> for FrameStackItem<V, E>
where
    I: FrameEngine,
    BlockFrame<V, E>: Frame<I, BlockFrame<V, E>, Self, Completion = Completion<V>>,
    CFGFrame<V, E>: Frame<I, CFGFrame<V, E>, Self, Completion = Completion<V>>,
    CallFrame<V>: Frame<I, CallFrame<V>, Self, Completion = Completion<V>>,
    DiGraphFrame<V, E>: Frame<I, DiGraphFrame<V, E>, Self, Completion = Completion<V>>,
    ScfIfFrame<V, E>: Frame<I, ScfIfFrame<V, E>, Self, Completion = Completion<V>>,
    ScfForFrame<V, E>: Frame<I, ScfForFrame<V, E>, Self, Completion = Completion<V>>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.step_into(interp)?.map_next(Self::Block)),
            Self::CFG(frame) => Ok(frame.step_into(interp)?.map_next(Self::CFG)),
            Self::Call(frame) => Ok(frame.step_into(interp)?.map_next(Self::Call)),
            Self::DiGraph(frame) => Ok(frame.step_into(interp)?.map_next(Self::DiGraph)),
            Self::ScfIf(frame) => Ok(frame.step_into(interp)?.map_next(Self::ScfIf)),
            Self::ScfFor(frame) => Ok(frame.step_into(interp)?.map_next(Self::ScfFor)),
        }
    }

    fn resume_done_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::Block)),
            Self::CFG(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::CFG)),
            Self::Call(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::Call)),
            Self::DiGraph(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::DiGraph)),
            Self::ScfIf(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::ScfIf)),
            Self::ScfFor(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::ScfFor)),
        }
    }

    fn resume_into(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.resume_into(completion, interp)?.map_next(Self::Block)),
            Self::CFG(frame) => Ok(frame.resume_into(completion, interp)?.map_next(Self::CFG)),
            Self::Call(frame) => Ok(frame.resume_into(completion, interp)?.map_next(Self::Call)),
            Self::DiGraph(frame) => Ok(frame
                .resume_into(completion, interp)?
                .map_next(Self::DiGraph)),
            Self::ScfIf(frame) => Ok(frame.resume_into(completion, interp)?.map_next(Self::ScfIf)),
            Self::ScfFor(frame) => Ok(frame
                .resume_into(completion, interp)?
                .map_next(Self::ScfFor)),
        }
    }
}

// ===========================================================================
// Abstract
// ===========================================================================

/// Abstract total frame: standard abstract traversal plus the SCF if/for frames.
///
/// No `AbstractDiGraphFrame` variant, so the derive omits `from_digraph` and the
/// trait's refusing default applies — toy-lang has no graph bodies.
#[derive(AbstractFrameBuild)]
pub enum ToyAbstractFrame<V, E, K> {
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
    ScfIf(AbstractScfIfFrame<V, E, K>),
    ScfFor(AbstractScfForFrame<V, E, K>),
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
    I: ForwardDataflowFrameEngine<Value = V, Error = E, SummaryKey = K>
        + SparseForwardInterp<Frame = F>,
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
    DenseBackwardCompletion, DenseBackwardFrameEngine, DenseBlockFrame, DenseFrameBuild,
};
use kirin_scf::{BuildDenseScfFor, BuildDenseScfIf, DenseScfForFrame, DenseScfIfFrame};

use kirin::prelude::Lattice;

/// Dense backward total frame: the standard block walk plus the SCF dense
/// frames (arm-join for `scf.if`, the loop-carried fixpoint for `scf.for`).
#[derive(DenseFrameBuild)]
pub enum ToyDenseBackwardFrame<V, E> {
    Block(DenseBlockFrame<V, E>),
    ScfIf(DenseScfIfFrame<V, E>),
    ScfFor(DenseScfForFrame<V, E>),
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
    I: DenseBackwardFrameEngine<Value = V, Error = E, Frame = F>,
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
