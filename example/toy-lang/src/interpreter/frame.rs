//! Total frame types for the toy language.
//!
//! The toy language uses `kirin-scf`, whose `scf.for` pushes a dialect-owned
//! loop frame ([`ScfForFrame`]/[`AbstractScfForFrame`]). A language that uses
//! such a dialect explicitly composes the standard framework frames — the representation walkers
//! ([`BlockFrame`]/[`CFGFrame`]/[`DiGraphFrame`]) and the [`CallFrame`] call
//! boundary — plus the dialect frames. Each total stack-item enum is a
//! composition root; its member frames do not know which enum stores them.

use kirin_interpreter::engine::{
    AbstractBlockFrame, AbstractCallFrame, AbstractCompletion, AbstractDiGraphFrame, BlockFrame,
    CFGFrame, CallFrame, CallRequest, Completion, DiGraphFrame, Frame, FrameEffect, FrameEngine,
};
use kirin_scf::{AbstractScfForFrame, AbstractScfIfFrame, ScfForFrame, ScfIfFrame};

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
    BlockFrame<V, E>: Frame<I, Self, Completion = Completion<V>>,
    CFGFrame<V, E>: Frame<I, Self, Completion = Completion<V>>,
    CallFrame<V>: Frame<I, Self, Completion = Completion<V>>,
    DiGraphFrame<V, E>: Frame<I, Self, Completion = Completion<V>>,
    ScfIfFrame<V, E>: Frame<I, Self, Completion = Completion<V>>,
    ScfForFrame<V, E>: Frame<I, Self, Completion = Completion<V>>,
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

/// Toy's abstract stack-item composition: framework traversal plus SCF.
///
/// The graph variant is present because the generic sparse-forward engine can
/// select a graph body even though the current toy programs use CFG bodies.
pub enum ToyAbstractFrame<V, E, K> {
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
    DiGraph(AbstractDiGraphFrame<V, E, K>),
    ScfIf(AbstractScfIfFrame<V, E, K>),
    ScfFor(AbstractScfForFrame<V, E, K>),
}

impl<V, E, K> From<AbstractBlockFrame<V, E, K>> for ToyAbstractFrame<V, E, K> {
    fn from(frame: AbstractBlockFrame<V, E, K>) -> Self {
        Self::Block(frame)
    }
}

impl<V, E, K> From<AbstractCallFrame<V, E, K>> for ToyAbstractFrame<V, E, K> {
    fn from(frame: AbstractCallFrame<V, E, K>) -> Self {
        Self::Call(frame)
    }
}

impl<V, E, K> From<AbstractDiGraphFrame<V, E, K>> for ToyAbstractFrame<V, E, K> {
    fn from(frame: AbstractDiGraphFrame<V, E, K>) -> Self {
        Self::DiGraph(frame)
    }
}

impl<V, E, K> From<AbstractScfIfFrame<V, E, K>> for ToyAbstractFrame<V, E, K> {
    fn from(frame: AbstractScfIfFrame<V, E, K>) -> Self {
        Self::ScfIf(frame)
    }
}

impl<V, E, K> From<AbstractScfForFrame<V, E, K>> for ToyAbstractFrame<V, E, K> {
    fn from(frame: AbstractScfForFrame<V, E, K>) -> Self {
        Self::ScfFor(frame)
    }
}

impl<I, V, E, K> Frame<I> for ToyAbstractFrame<V, E, K>
where
    I: FrameEngine,
    AbstractBlockFrame<V, E, K>: Frame<I, Self, Completion = AbstractCompletion<V>>,
    AbstractCallFrame<V, E, K>: Frame<I, Self, Completion = AbstractCompletion<V>>,
    AbstractDiGraphFrame<V, E, K>: Frame<I, Self, Completion = AbstractCompletion<V>>,
    AbstractScfIfFrame<V, E, K>: Frame<I, Self, Completion = AbstractCompletion<V>>,
    AbstractScfForFrame<V, E, K>: Frame<I, Self, Completion = AbstractCompletion<V>>,
{
    type Completion = AbstractCompletion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.step_into(interp)?.map_next(Self::Block)),
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
// Dense backward (classic per-point liveness)
// ===========================================================================

use kirin_interpreter::engine::{DenseBackwardCompletion, DenseBlockFrame};
use kirin_scf::{DenseScfForFrame, DenseScfIfFrame};

/// Toy's private dense-backward stack item: the reverse block walk plus the
/// SCF dense continuations (arm join and loop-carried fixpoint).
pub(crate) enum ToyDenseBackwardFrame<V, E> {
    Block(DenseBlockFrame<V, E>),
    ScfIf(DenseScfIfFrame<V, E>),
    ScfFor(DenseScfForFrame<V, E>),
}

impl<V, E> From<DenseBlockFrame<V, E>> for ToyDenseBackwardFrame<V, E> {
    fn from(frame: DenseBlockFrame<V, E>) -> Self {
        Self::Block(frame)
    }
}

impl<V, E> From<DenseScfIfFrame<V, E>> for ToyDenseBackwardFrame<V, E> {
    fn from(frame: DenseScfIfFrame<V, E>) -> Self {
        Self::ScfIf(frame)
    }
}

impl<V, E> From<DenseScfForFrame<V, E>> for ToyDenseBackwardFrame<V, E> {
    fn from(frame: DenseScfForFrame<V, E>) -> Self {
        Self::ScfFor(frame)
    }
}

impl<I, V, E> Frame<I> for ToyDenseBackwardFrame<V, E>
where
    I: FrameEngine,
    DenseBlockFrame<V, E>: Frame<I, Self, Completion = DenseBackwardCompletion<V>>,
    DenseScfIfFrame<V, E>: Frame<I, Self, Completion = DenseBackwardCompletion<V>>,
    DenseScfForFrame<V, E>: Frame<I, Self, Completion = DenseBackwardCompletion<V>>,
{
    type Completion = DenseBackwardCompletion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.step_into(interp)?.map_next(Self::Block)),
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
            Self::ScfIf(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::ScfIf)),
            Self::ScfFor(frame) => Ok(frame.resume_done_into(interp)?.map_next(Self::ScfFor)),
        }
    }

    fn resume_into(
        self,
        completion: DenseBackwardCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.resume_into(completion, interp)?.map_next(Self::Block)),
            Self::ScfIf(frame) => Ok(frame.resume_into(completion, interp)?.map_next(Self::ScfIf)),
            Self::ScfFor(frame) => Ok(frame
                .resume_into(completion, interp)?
                .map_next(Self::ScfFor)),
        }
    }
}
