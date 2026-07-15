use crate::{Frame, FrameDriver, FrameEffect, InterpreterError, SparseForwardInterp};

use super::{BlockFrame, CallFrame, CfgFrame, Completion, DiGraphFrame, FrameBuild};

/// The standard total concrete frame enum: the representation walkers plus
/// the call boundary, no structured-control dialect frames and no
/// callable-UnGraph policy (so a call into an `UnGraph` body reports
/// [`NoDefaultWalker`](InterpreterError::NoDefaultWalker)).
pub enum StandardFrame<V, E> {
    Block(BlockFrame<V, E>),
    Cfg(CfgFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
}

impl<V, E> FrameBuild<V, E> for StandardFrame<V, E> {
    fn from_block(frame: BlockFrame<V, E>) -> Self {
        StandardFrame::Block(frame)
    }
    fn from_cfg(frame: CfgFrame<V, E>) -> Self {
        StandardFrame::Cfg(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        StandardFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self {
        StandardFrame::DiGraph(frame)
    }
}

impl<I, V, E> Frame<I> for StandardFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = StandardFrame<V, E>>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Block(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::Cfg(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::Call(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::DiGraph(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Block(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardFrame::Cfg(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(I::Error::from),
            StandardFrame::DiGraph(frame) => Ok(frame.resume_done_into::<Self>()),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Block(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardFrame::Cfg(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardFrame::DiGraph(frame) => frame.resume_into::<I, Self>(completion, interp),
        }
    }
}
