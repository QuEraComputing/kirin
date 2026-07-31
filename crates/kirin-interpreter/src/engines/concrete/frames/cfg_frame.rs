use kirin_ir::{CFG, CompileStage, Product};

use crate::{
    EnvIndex, Frame, FrameDriver, FrameEffect, InterpreterError, SparseForwardEffect,
    SparseForwardInterp,
};

use super::block_cursor::BlockCursor;
use super::{CallFrame, Completion, FrameBuild};

/// Representation walker for a [`CFG`]: enter the entry block, run
/// statements, follow `Jump` edges between blocks (binding successor
/// arguments and resetting the cursor), and surface the exit through the
/// [`Completion`] protocol.
///
/// Traversal mechanics only. A `CFGFrame` does not own an activation, is not
/// a call boundary, and does not decide whether a `Return` belongs to a
/// function call — it completes [`Returned`](Completion::Returned) and lets
/// the completion bubble to the nearest [`CallFrame`]. An undecided concrete
/// `Branch` is an error (single-path execution); exploring branch
/// alternatives is the abstract engine's business.
pub struct CFGFrame<V, E> {
    stage: CompileStage,
    index: EnvIndex,
    cfg: CFG,
    /// Entry arguments awaiting the first step (the entry block is resolved
    /// lazily, so construction needs no engine access).
    pending: Option<Product<V>>,
    /// The active block cursor; `None` until the entry block is resolved.
    cursor: Option<BlockCursor<V>>,
    _marker: std::marker::PhantomData<fn() -> E>,
}

impl<V, E> CFGFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    /// Walk `cfg` from its entry block, binding `args` to the entry block's
    /// parameters on the first step. Pure construction — needs no engine
    /// access.
    pub fn new(stage: CompileStage, index: EnvIndex, cfg: CFG, args: Product<V>) -> Self {
        Self {
            stage,
            index,
            cfg,
            pending: Some(args),
            cursor: None,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<I, F, V, E> Frame<I, F> for CFGFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = F>,
    F: FrameBuild<V, E>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    /// Execute the next statement and translate its [`SparseForwardEffect`]
    /// into a [`FrameEffect`] over the total frame type `F`.
    fn step_into(mut self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        // First step: find the entry block and bind the entry arguments.
        if let Some(args) = self.pending.take() {
            let entry = interp
                .cfg_entry(self.stage, self.cfg)?
                .ok_or_else(|| E::from(InterpreterError::EmptyCFG))?;
            let mut cursor = BlockCursor::new(self.stage, self.index, entry, args);
            cursor.bind_entry(interp)?;
            self.cursor = Some(cursor);
            return Ok(FrameEffect::Continue(F::from_cfg(self)));
        }
        let cursor = self
            .cursor
            .as_mut()
            .ok_or_else(|| E::from(InterpreterError::Custom("cfg frame stepped before entry")))?;
        let Some(statement) = cursor.advance(interp)? else {
            return Err(E::from(InterpreterError::BlockFellThrough(cursor.block)));
        };

        match interp.run_statement(self.stage, statement, self.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(F::from_cfg(self))),
            SparseForwardEffect::Jump(edge) => {
                cursor.enter_block(interp, edge.target, &edge.args)?;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            SparseForwardEffect::Branch(_) => Err(E::from(InterpreterError::IndeterminateBranch)),
            SparseForwardEffect::Push { frame, results } => {
                cursor.expect_results(results);
                Ok(FrameEffect::Push {
                    parent: F::from_cfg(self),
                    child: frame,
                })
            }
            SparseForwardEffect::Call(call) => {
                let pending = CallFrame::pending(self.stage, self.index, call);
                Ok(FrameEffect::Push {
                    parent: F::from_cfg(self),
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
        Ok(FrameEffect::Continue(F::from_cfg(self)))
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
                let cursor = self.cursor.as_mut().ok_or_else(|| {
                    E::from(InterpreterError::Custom("cfg frame resumed before entry"))
                })?;
                cursor.write_child_results(interp, values)?;
                Ok(FrameEffect::Continue(F::from_cfg(self)))
            }
            Completion::Returned(values) => Ok(FrameEffect::Complete(Completion::Returned(values))),
        }
    }
}
