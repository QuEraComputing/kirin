use kirin_ir::{Block, CompileStage, Product};

use crate::{
    BlockQueries, EnvIndex, Frame, FrameEffect, InterpreterError, SparseForwardEffect,
    SparseForwardInterp, StatementDispatch,
};

use super::block_cursor::BlockCursor;
use super::{CallFrame, Completion, FrameBuild};

/// Representation walker for exactly one [`Block`]: bind its parameters,
/// run its statements in order, and surface the exit through the
/// [`Completion`] protocol — `Return` as
/// [`Returned`](Completion::Returned), `Yield` as
/// [`Yielded`](Completion::Yielded).
///
/// Traversal mechanics only. A `BlockFrame` does not own an activation, is
/// not a call boundary, and does not know whether it is a callable function
/// body ([`CallFrame`] → `BlockFrame`) or a nested structured-operation body
/// (dialect frame → `BlockFrame`): the parent frame defines the role and
/// interprets the completion. CFG transitions (`Jump`/`Branch`) are rejected
/// — a single block owns no CFG edges; multi-block traversal is
/// [`CFGFrame`](super::CFGFrame)'s job.
pub struct BlockFrame<V, E> {
    cursor: BlockCursor<V>,
    _marker: std::marker::PhantomData<fn() -> E>,
}

impl<V, E> BlockFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    /// Walk `block`, binding `args` to its parameters on the first step.
    /// Pure construction — needs no engine access, so a dialect frame can
    /// build one as plain values.
    pub fn new(stage: CompileStage, index: EnvIndex, block: Block, args: Product<V>) -> Self {
        Self {
            cursor: BlockCursor::new(stage, index, block, args),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<I, F, V, E> Frame<I, F> for BlockFrame<V, E>
where
    I: BlockQueries<Value = V, Error = E> + StatementDispatch + SparseForwardInterp<Frame = F>,
    F: FrameBuild<V, E>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    /// Execute the next statement and translate its [`SparseForwardEffect`]
    /// into a [`FrameEffect`] over the total frame type `F`.
    fn step_into(mut self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        if self.cursor.bind_entry(interp)? {
            return Ok(FrameEffect::Continue(F::from_block(self)));
        }
        let Some(statement) = self.cursor.advance(interp)? else {
            return Err(E::from(InterpreterError::BlockFellThrough(
                self.cursor.block,
            )));
        };

        match interp.run_statement(self.cursor.stage, statement, self.cursor.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(F::from_block(self))),
            SparseForwardEffect::Jump(_) | SparseForwardEffect::Branch(_) => {
                Err(E::from(InterpreterError::CFGControlFlowInStructuredBody))
            }
            SparseForwardEffect::Push { frame, results } => {
                self.cursor.expect_results(results);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: frame,
                })
            }
            SparseForwardEffect::Call(call) => {
                let pending = CallFrame::pending(self.cursor.stage, self.cursor.index, call);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: F::from_call(pending),
                })
            }
            SparseForwardEffect::Yield(values) => {
                Ok(FrameEffect::Complete(Completion::Yielded(values)))
            }
            SparseForwardEffect::Return(values) => {
                Ok(FrameEffect::Complete(Completion::Returned(values)))
            }
        }
    }

    /// A child finished without a payload (its results are already in the
    /// shared activation, e.g. a returned call): resume at the advanced
    /// cursor.
    fn resume_done_into(self, _interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        Ok(FrameEffect::Continue(F::from_block(self)))
    }

    /// A child bubbled a completion: a pushed frame's values land in the
    /// push's result slots; a `Returned` keeps bubbling toward the nearest
    /// [`CallFrame`] (this frame owns no activation to free).
    fn resume_into(
        mut self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E> {
        match completion {
            Completion::Finished(values) | Completion::Yielded(values) => {
                self.cursor.write_child_results(interp, values)?;
                Ok(FrameEffect::Continue(F::from_block(self)))
            }
            Completion::Returned(values) => Ok(FrameEffect::Complete(Completion::Returned(values))),
        }
    }
}
