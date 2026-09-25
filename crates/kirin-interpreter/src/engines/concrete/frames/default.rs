//! Default concrete frame-stack composition.
//!
//! Languages that need additional continuations define their own private
//! `FrameStackItem` enum instead of extending this closed default.

use crate::{Frame, FrameEffect, FrameEngine};

use super::{BlockFrame, CFGFrame, CallFrame, CallRequest, Completion, DiGraphFrame};

/// Private homogeneous stack element for the default concrete interpreter.
pub(crate) enum FrameStackItem<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
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

impl<I, V, E> Frame<I> for FrameStackItem<V, E>
where
    I: FrameEngine,
    BlockFrame<V, E>: Frame<I, FrameStackItem<V, E>, Completion = Completion<V>>,
    CFGFrame<V, E>: Frame<I, FrameStackItem<V, E>, Completion = Completion<V>>,
    CallFrame<V>: Frame<I, FrameStackItem<V, E>, Completion = Completion<V>>,
    DiGraphFrame<V, E>: Frame<I, FrameStackItem<V, E>, Completion = Completion<V>>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            Self::Block(frame) => Ok(frame.step_into(interp)?.map_next(Self::Block)),
            Self::CFG(frame) => Ok(frame.step_into(interp)?.map_next(Self::CFG)),
            Self::Call(frame) => Ok(frame.step_into(interp)?.map_next(Self::Call)),
            Self::DiGraph(frame) => Ok(frame.step_into(interp)?.map_next(Self::DiGraph)),
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
        }
    }
}
