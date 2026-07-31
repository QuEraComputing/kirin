use crate::{Frame, FrameDriver, FrameEffect, InterpreterError, SparseForwardInterp};

use super::{BlockFrame, CFGFrame, CallFrame, Completion, DiGraphFrame, FrameBuild};

/// The standard total concrete frame enum: the representation walkers plus
/// the call boundary, no structured-control dialect frames and no
/// callable-UnGraph policy (so a call into an `UnGraph` body reports
/// [`NoDefaultWalker`](InterpreterError::NoDefaultWalker)).
pub enum StandardFrame<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
}

impl<V, E> FrameBuild<V, E> for StandardFrame<V, E> {
    fn from_block(frame: BlockFrame<V, E>) -> Self {
        StandardFrame::Block(frame)
    }
    fn from_cfg(frame: CFGFrame<V, E>) -> Self {
        StandardFrame::CFG(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        StandardFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self {
        StandardFrame::DiGraph(frame)
    }
}

/// A *universe* impl: generic over the outer total frame type `F` so that
/// `StandardFrame` can be the stack's element type (`F = Self`, the usual case)
/// **or** be embedded in a larger enum — an instrumenting wrapper, say — without
/// re-enumerating its variants.
impl<I, F, V, E> Frame<I, F> for StandardFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = F>,
    F: FrameBuild<V, E>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            StandardFrame::Block(frame) => frame.step_into(interp),
            StandardFrame::CFG(frame) => frame.step_into(interp),
            StandardFrame::Call(frame) => frame.step_into(interp),
            StandardFrame::DiGraph(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            StandardFrame::Block(frame) => frame.resume_done_into(interp),
            StandardFrame::CFG(frame) => frame.resume_done_into(interp),
            StandardFrame::Call(frame) => frame.resume_done_into(interp),
            StandardFrame::DiGraph(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self {
            StandardFrame::Block(frame) => frame.resume_into(completion, interp),
            StandardFrame::CFG(frame) => frame.resume_into(completion, interp),
            StandardFrame::Call(frame) => frame.resume_into(completion, interp),
            StandardFrame::DiGraph(frame) => frame.resume_into(completion, interp),
        }
    }
}
