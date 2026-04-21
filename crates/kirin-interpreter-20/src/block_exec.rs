use kirin_ir::Block;

use crate::control::Control;
use crate::env::Env;

/// Outcome of a jump during block execution.
///
/// Concrete interpreters rewind the cursor in-place; abstract interpreters
/// enqueue the target block and pop the cursor.
pub enum JumpOutcome<V, Ext> {
    /// Concrete: cursor state updated in-place by the caller; keep executing.
    Rewound,
    /// Abstract: return this control signal and stop.
    Done(Control<V, Ext>),
}

/// Abstracts the concrete/abstract difference in block execution.
///
/// A single `BlockCursor<V, L>` implements `Execute<E>` for all `E: BlockExecEnv`,
/// with concrete/abstract behavior determined by `E`'s implementation.
pub trait BlockExecEnv: Env {
    /// Handle a control-flow jump.
    fn exec_jump(
        &mut self,
        target: Block,
        args: Vec<Self::Value>,
    ) -> JumpOutcome<Self::Value, Self::Ext>;

    /// Handle a non-deterministic fork.
    fn exec_fork(
        &mut self,
        branches: Vec<(Block, Vec<Self::Value>)>,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;

    /// Called when a block's statements are exhausted.
    fn exec_block_end(&self) -> Control<Self::Value, Self::Ext>;
}
