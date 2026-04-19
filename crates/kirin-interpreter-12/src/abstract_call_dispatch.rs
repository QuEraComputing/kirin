use kirin_ir::{Block, CompileStage, Pipeline, SpecializedFunction, StageMeta};

use crate::error::InterpreterError;

/// Cursor-creation hook for abstract cross-stage calls.
///
/// Mirrors [`CallDispatch`][crate::call_dispatch::CallDispatch] for the concrete
/// interpreter. Implement on your stage container `S` to control how
/// `AbstractInterp` creates block cursors and resolves entry blocks when
/// handling `Control::Call`. Enables:
///
/// - **Single-stage**: always create `AbstractBlockCursor<V, L>` directly.
/// - **Multi-stage**: match on `stage_id` and create the appropriate abstract
///   cursor variant.
pub trait AbstractCallDispatch<V, C>: StageMeta + Sized {
    /// Create an abstract block cursor for the given `block` at `stage_id`.
    ///
    /// Implementations match on `stage_id` (via `pipeline.stage(stage_id)`) to
    /// decide which cursor variant to wrap the block in.
    fn make_abstract_cursor(
        pipeline: &Pipeline<Self>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> C;

    /// Find the entry block of `callee` at `stage_id`.
    ///
    /// For single-stage interpreters this delegates to
    /// [`PipelineHandle::entry_block_of`][crate::pipeline::PipelineHandle::entry_block_of]
    /// with a fixed dialect.  Multi-stage implementors match on `stage_id`.
    fn entry_block_for(
        pipeline: &Pipeline<Self>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError>;
}
